//! Accesso al pannello: sessioni in memoria (un riavvio del nodo richiede un
//! nuovo login), login a due passi con TOTP, blocco dopo troppi errori.
//! Gli utenti e i loro scope stanno in `users.rs`; qui non si copiano: a ogni
//! richiesta si rilegge l'utente, così disabilitarlo o cambiargli i permessi
//! vale subito.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use argon2::password_hash::rand_core::{OsRng, RngCore};

use crate::totp;
use crate::users::{self, Store, User};

pub const COOKIE: &str = "otr_session";
pub const SESSION_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const CHALLENGE_TTL: Duration = Duration::from_secs(5 * 60);
const CHALLENGE_TRIES: u32 = 5;
const MAX_FAILS: usize = 5;
const FAIL_WINDOW: Duration = Duration::from_secs(5 * 60);
const LOCK_FOR: Duration = Duration::from_secs(5 * 60);

/// Cosa deve fare l'utente prima di poter usare il pannello.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    ChangePassword,
    SetupTwoFactor,
}

/// Chi sta facendo la richiesta, con i permessi di adesso.
#[derive(Debug, Clone)]
pub struct Principal {
    pub token: String,
    pub id: String,
    pub username: String,
    pub role: String,
    pub scopes: BTreeSet<String>,
    pub two_factor: bool,
    pub requirement: Option<Requirement>,
}

impl Principal {
    pub fn can(&self, scope: &str) -> bool {
        self.scopes.contains(scope)
    }
}

pub enum Login {
    Session(String),
    SecondFactor(String),
    Invalid,
    Locked(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum SecondFactorError {
    Invalid,
    Expired,
    Locked(u64),
}

struct Session {
    user_id: String,
    expires: Instant,
}

struct Challenge {
    user_id: String,
    expires: Instant,
    tries: u32,
}

#[derive(Default)]
struct Fails {
    recent: Vec<Instant>,
    locked_until: Option<Instant>,
}

pub struct Auth {
    pub users: Store,
    sessions: Mutex<HashMap<String, Session>>,
    challenges: Mutex<HashMap<String, Challenge>>,
    fails: Mutex<HashMap<String, Fails>>,
}

fn random_token() -> String {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    hex::encode(raw)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Hash di un utente inesistente: si verifica comunque, così il tempo di
/// risposta non rivela se il nome esiste.
fn dummy_hash() -> &'static str {
    static H: OnceLock<String> = OnceLock::new();
    H.get_or_init(|| users::hash_password("password-inesistente").unwrap_or_default())
}

pub fn token_from(cookie_header: Option<&str>) -> Option<&str> {
    cookie_header?
        .split(';')
        .filter_map(|c| c.trim().strip_prefix(COOKIE)?.strip_prefix('='))
        .next()
}

impl Auth {
    pub fn new(state_dir: &Path) -> Self {
        Auth {
            users: Store::load(state_dir),
            sessions: Mutex::new(HashMap::new()),
            challenges: Mutex::new(HashMap::new()),
            fails: Mutex::new(HashMap::new()),
        }
    }

    /// Finché non esiste nessun utente il pannello mostra la creazione del primo.
    pub fn setup_required(&self) -> bool {
        self.users.is_empty()
    }

    /// Crea il primo utente (Amministratore). Il chiamante tiene il lock di
    /// scrittura, così due richieste contemporanee non ne creano due.
    pub async fn create_first_admin(&self, username: &str, password: &str) -> Result<User, String> {
        if !self.setup_required() {
            return Err("l'amministratore esiste già".into());
        }
        let username = users::check_username(username)?;
        users::check_password(password)?;
        let pw = password.to_owned();
        let hash = tokio::task::spawn_blocking(move || users::hash_password(&pw))
            .await
            .map_err(|e| e.to_string())??;
        self.users.insert(User {
            id: users::new_id(),
            username,
            password_hash: hash,
            role: "admin".into(),
            scopes: vec![],
            disabled: false,
            must_change_password: false,
            totp: Default::default(),
            recovery_hashes: vec![],
            created_at: users::now_iso(),
            last_login_at: None,
        })
    }

    // --- blocco dopo troppi errori ---------------------------------------

    fn key(name: &str) -> String {
        name.trim().to_lowercase()
    }

    /// Secondi residui di blocco, se l'utente è bloccato.
    pub fn locked_for(&self, name: &str) -> Option<u64> {
        self.locked_at(name, Instant::now())
    }

    fn locked_at(&self, name: &str, now: Instant) -> Option<u64> {
        let f = self.fails.lock().unwrap();
        let until = f.get(&Self::key(name))?.locked_until?;
        (until > now).then(|| (until - now).as_secs().max(1))
    }

    pub fn note_fail(&self, name: &str) {
        self.note_fail_at(name, Instant::now());
    }

    fn note_fail_at(&self, name: &str, now: Instant) {
        let mut f = self.fails.lock().unwrap();
        let e = f.entry(Self::key(name)).or_default();
        e.recent.retain(|t| now.duration_since(*t) < FAIL_WINDOW);
        e.recent.push(now);
        if e.recent.len() >= MAX_FAILS {
            e.locked_until = Some(now + LOCK_FOR);
            e.recent.clear();
        }
    }

    pub fn note_success(&self, name: &str) {
        self.fails.lock().unwrap().remove(&Self::key(name));
    }

    // --- login -----------------------------------------------------------

    pub async fn login_password(&self, username: &str, password: &str) -> Login {
        if let Some(s) = self.locked_for(username) {
            return Login::Locked(s);
        }
        let user = self.users.find_by_name(username);
        let hash = user
            .as_ref()
            .map_or_else(|| dummy_hash().to_owned(), |u| u.password_hash.clone());
        let pw = password.to_owned();
        let ok = tokio::task::spawn_blocking(move || users::verify_password(&pw, &hash))
            .await
            .unwrap_or(false);
        let Some(user) = user.filter(|u| ok && !u.disabled) else {
            self.note_fail(username);
            return Login::Invalid;
        };
        if user.totp.enabled {
            let token = random_token();
            self.challenges.lock().unwrap().insert(
                token.clone(),
                Challenge {
                    user_id: user.id.clone(),
                    expires: Instant::now() + CHALLENGE_TTL,
                    tries: 0,
                },
            );
            return Login::SecondFactor(token);
        }
        Login::Session(self.finish_login(&user))
    }

    /// Secondo passo: codice TOTP oppure codice di recupero (monouso).
    pub async fn login_second_factor(
        &self,
        challenge: &str,
        code: Option<&str>,
        recovery: Option<&str>,
    ) -> Result<String, SecondFactorError> {
        let user_id = {
            let mut c = self.challenges.lock().unwrap();
            let Some(ch) = c.get_mut(challenge) else {
                return Err(SecondFactorError::Expired);
            };
            if ch.expires <= Instant::now() {
                c.remove(challenge);
                return Err(SecondFactorError::Expired);
            }
            ch.tries += 1;
            if ch.tries > CHALLENGE_TRIES {
                c.remove(challenge);
                return Err(SecondFactorError::Invalid);
            }
            ch.user_id.clone()
        };
        let Some(user) = self
            .users
            .get(&user_id)
            .filter(|u| !u.disabled && u.totp.enabled)
        else {
            return Err(SecondFactorError::Expired);
        };
        if let Some(s) = self.locked_for(&user.username) {
            return Err(SecondFactorError::Locked(s));
        }

        let accepted = if let Some(code) = code {
            match user
                .totp
                .secret
                .as_deref()
                .and_then(|s| totp::verify(s, code, now_secs(), user.totp.last_step))
            {
                Some(step) => self
                    .users
                    .update(&user.id, |u| {
                        u.totp.last_step = step;
                        Ok(())
                    })
                    .is_ok(),
                None => false,
            }
        } else if let Some(rc) = recovery {
            let h = totp::hash_recovery_code(rc);
            user.recovery_hashes.contains(&h)
                && self
                    .users
                    .update(&user.id, |u| {
                        u.recovery_hashes.retain(|x| *x != h);
                        Ok(())
                    })
                    .is_ok()
        } else {
            false
        };

        if !accepted {
            self.note_fail(&user.username);
            return Err(SecondFactorError::Invalid);
        }
        self.challenges.lock().unwrap().remove(challenge);
        Ok(self.finish_login(&user))
    }

    fn finish_login(&self, user: &User) -> String {
        let _ = self.users.update(&user.id, |u| {
            u.last_login_at = Some(users::now_iso());
            Ok(())
        });
        self.note_success(&user.username);
        self.start_session(&user.id)
    }

    // --- sessioni --------------------------------------------------------

    pub fn start_session(&self, user_id: &str) -> String {
        let token = random_token();
        let mut s = self.sessions.lock().unwrap();
        let now = Instant::now();
        s.retain(|_, x| x.expires > now);
        s.insert(
            token.clone(),
            Session {
                user_id: user_id.to_owned(),
                expires: now + SESSION_TTL,
            },
        );
        self.challenges
            .lock()
            .unwrap()
            .retain(|_, c| c.expires > now);
        token
    }

    pub fn end_session(&self, cookie_header: Option<&str>) {
        if let Some(t) = token_from(cookie_header) {
            self.sessions.lock().unwrap().remove(t);
        }
    }

    /// Chiude tutte le sessioni di un utente (tranne, eventualmente, una).
    pub fn revoke_user_sessions(&self, user_id: &str, except: Option<&str>) {
        self.sessions
            .lock()
            .unwrap()
            .retain(|t, s| s.user_id != user_id || Some(t.as_str()) == except);
    }

    /// L'utente della sessione, con i permessi attuali. `None` se la sessione
    /// non c'è, è scaduta o l'utente è stato eliminato o disabilitato.
    pub fn principal(&self, cookie_header: Option<&str>) -> Option<Principal> {
        let token = token_from(cookie_header)?.to_owned();
        let user_id = {
            let s = self.sessions.lock().unwrap();
            let sess = s.get(&token)?;
            if sess.expires <= Instant::now() {
                return None;
            }
            sess.user_id.clone()
        };
        let user = self.users.get(&user_id).filter(|u| !u.disabled)?;
        let requirement = if user.must_change_password {
            Some(Requirement::ChangePassword)
        } else if self.users.policy().requires(&user) && !user.totp.enabled {
            Some(Requirement::SetupTwoFactor)
        } else {
            None
        };
        Some(Principal {
            token,
            id: user.id.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
            scopes: user.effective_scopes(),
            two_factor: user.totp.enabled,
            requirement,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn auth() -> (tempfile::TempDir, Auth) {
        let dir = tempfile::tempdir().unwrap();
        let a = Auth::new(dir.path());
        (dir, a)
    }

    #[test]
    fn cookie_parsing() {
        assert_eq!(token_from(Some("a=1; otr_session=abc; b=2")), Some("abc"));
        assert_eq!(token_from(Some("xotr_session=abc")), None);
        assert_eq!(token_from(None), None);
    }

    #[tokio::test]
    async fn first_admin_login_and_sessions() {
        let (_d, a) = auth();
        assert!(a.setup_required());
        assert!(a.create_first_admin("admin", "corta").await.is_err());
        a.create_first_admin("admin", "una-password-lunga")
            .await
            .unwrap();
        assert!(a
            .create_first_admin("altro", "una-password-lunga")
            .await
            .is_err());

        assert!(matches!(
            a.login_password("admin", "sbagliata!!").await,
            Login::Invalid
        ));
        assert!(matches!(
            a.login_password("nessuno", "una-password-lunga").await,
            Login::Invalid
        ));
        let Login::Session(t) = a.login_password("Admin", "una-password-lunga").await else {
            panic!("login non riuscito")
        };
        let cookie = format!("{COOKIE}={t}");
        let p = a.principal(Some(&cookie)).unwrap();
        assert_eq!(p.username, "admin");
        assert!(p.can("users:manage") && p.can("domains:read"));
        assert!(p.requirement.is_none());
        a.end_session(Some(&cookie));
        assert!(a.principal(Some(&cookie)).is_none());
    }

    #[tokio::test]
    async fn disabled_user_is_cut_off_immediately() {
        let (_d, a) = auth();
        let admin = a
            .create_first_admin("admin", "una-password-lunga")
            .await
            .unwrap();
        let hash = users::hash_password("una-password-lunga").unwrap();
        let bob = a
            .users
            .insert(User {
                id: users::new_id(),
                username: "bob".into(),
                password_hash: hash,
                role: "viewer".into(),
                scopes: vec![],
                disabled: false,
                must_change_password: false,
                totp: Default::default(),
                recovery_hashes: vec![],
                created_at: users::now_iso(),
                last_login_at: None,
            })
            .unwrap();
        let Login::Session(t) = a.login_password("bob", "una-password-lunga").await else {
            panic!()
        };
        let cookie = format!("{COOKIE}={t}");
        assert!(a.principal(Some(&cookie)).is_some());
        // cambio di permessi: vale alla richiesta successiva
        a.users
            .update(&bob.id, |u| {
                u.role = "operator".into();
                Ok(())
            })
            .unwrap();
        assert!(a.principal(Some(&cookie)).unwrap().can("domains:write"));
        // disabilitato: fuori subito, e non rientra
        a.users
            .update(&bob.id, |u| {
                u.disabled = true;
                Ok(())
            })
            .unwrap();
        assert!(a.principal(Some(&cookie)).is_none());
        assert!(matches!(
            a.login_password("bob", "una-password-lunga").await,
            Login::Invalid
        ));
        let _ = admin;
    }

    #[tokio::test]
    async fn revoke_keeps_only_the_excepted_session() {
        let (_d, a) = auth();
        let u = a
            .create_first_admin("admin", "una-password-lunga")
            .await
            .unwrap();
        let t1 = a.start_session(&u.id);
        let t2 = a.start_session(&u.id);
        a.revoke_user_sessions(&u.id, Some(&t1));
        assert!(a.principal(Some(&format!("{COOKIE}={t1}"))).is_some());
        assert!(a.principal(Some(&format!("{COOKIE}={t2}"))).is_none());
    }

    #[test]
    fn lockout_after_five_failures_and_recovery() {
        let (_d, a) = auth();
        let t0 = Instant::now();
        for i in 0..4 {
            a.note_fail_at("Marco", t0 + Duration::from_secs(i));
            assert!(a.locked_at("marco", t0 + Duration::from_secs(i)).is_none());
        }
        a.note_fail_at("marco", t0 + Duration::from_secs(5));
        let left = a.locked_at("MARCO", t0 + Duration::from_secs(6)).unwrap();
        assert!((290..=300).contains(&left), "{left}");
        // dopo il blocco si può riprovare
        assert!(a
            .locked_at("marco", t0 + LOCK_FOR + Duration::from_secs(10))
            .is_none());
        // errori vecchi non si sommano
        a.note_fail_at("luca", t0);
        for i in 0..4 {
            a.note_fail_at("luca", t0 + FAIL_WINDOW + Duration::from_secs(10 + i));
        }
        assert!(a
            .locked_at("luca", t0 + FAIL_WINDOW + Duration::from_secs(20))
            .is_none());
    }

    #[tokio::test]
    async fn second_factor_flow_with_totp_and_recovery() {
        let (_d, a) = auth();
        let u = a
            .create_first_admin("admin", "una-password-lunga")
            .await
            .unwrap();
        let secret = totp::generate_secret();
        let codes = totp::generate_recovery_codes();
        let hashes: Vec<String> = codes.iter().map(|c| totp::hash_recovery_code(c)).collect();
        a.users
            .update(&u.id, |x| {
                x.totp.secret = Some(secret.clone());
                x.totp.enabled = true;
                x.recovery_hashes = hashes.clone();
                Ok(())
            })
            .unwrap();

        // la password da sola non basta: niente sessione, solo una challenge
        let Login::SecondFactor(ch) = a.login_password("admin", "una-password-lunga").await else {
            panic!("doveva chiedere il secondo fattore")
        };
        assert_eq!(
            a.login_second_factor(&ch, Some("000000"), None).await,
            Err(SecondFactorError::Invalid)
        );
        let good = totp::code_at(&secret, totp::step_of(now_secs())).unwrap();
        let t = a.login_second_factor(&ch, Some(&good), None).await.unwrap();
        assert!(
            a.principal(Some(&format!("{COOKIE}={t}")))
                .unwrap()
                .two_factor
        );
        // la challenge è consumata
        assert_eq!(
            a.login_second_factor(&ch, Some(&good), None).await,
            Err(SecondFactorError::Expired)
        );

        // lo stesso codice TOTP non si riusa in un secondo login
        let Login::SecondFactor(ch2) = a.login_password("admin", "una-password-lunga").await else {
            panic!()
        };
        assert_eq!(
            a.login_second_factor(&ch2, Some(&good), None).await,
            Err(SecondFactorError::Invalid)
        );

        // codice di recupero: vale una volta sola
        assert!(a
            .login_second_factor(&ch2, None, Some(&codes[0]))
            .await
            .is_ok());
        let Login::SecondFactor(ch3) = a.login_password("admin", "una-password-lunga").await else {
            panic!()
        };
        assert_eq!(
            a.login_second_factor(&ch3, None, Some(&codes[0])).await,
            Err(SecondFactorError::Invalid)
        );
        assert!(a
            .login_second_factor(&ch3, None, Some(&codes[1]))
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn challenge_allows_only_a_few_tries() {
        let (_d, a) = auth();
        let u = a
            .create_first_admin("admin", "una-password-lunga")
            .await
            .unwrap();
        a.users
            .update(&u.id, |x| {
                x.totp.secret = Some(totp::generate_secret());
                x.totp.enabled = true;
                Ok(())
            })
            .unwrap();
        let Login::SecondFactor(ch) = a.login_password("admin", "una-password-lunga").await else {
            panic!()
        };
        let mut last = Err(SecondFactorError::Invalid);
        for _ in 0..8 {
            last = a.login_second_factor(&ch, Some("111111"), None).await;
            if last == Err(SecondFactorError::Expired) {
                break;
            }
        }
        // dopo i tentativi concessi la challenge non esiste più (o l'utente è bloccato)
        assert!(matches!(
            last,
            Err(SecondFactorError::Expired) | Err(SecondFactorError::Locked(_))
        ));
    }

    #[tokio::test]
    async fn requirements_follow_policy_and_flags() {
        let (_d, a) = auth();
        let u = a
            .create_first_admin("admin", "una-password-lunga")
            .await
            .unwrap();
        let cookie = format!("{COOKIE}={}", a.start_session(&u.id));
        assert!(a.principal(Some(&cookie)).unwrap().requirement.is_none());
        a.users
            .set_policy(users::Policy {
                require_2fa: "all".into(),
            })
            .unwrap();
        assert_eq!(
            a.principal(Some(&cookie)).unwrap().requirement,
            Some(Requirement::SetupTwoFactor)
        );
        a.users
            .update(&u.id, |x| {
                x.must_change_password = true;
                Ok(())
            })
            .unwrap();
        assert_eq!(
            a.principal(Some(&cookie)).unwrap().requirement,
            Some(Requirement::ChangePassword)
        );
    }
}
