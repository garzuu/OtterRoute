//! Utenti del pannello: ruoli, scope sulle azioni, criterio di sicurezza e
//! archivio su disco (`state/users.json`, permessi 0600).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use argon2::password_hash::rand_core::{OsRng, RngCore};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};

pub const MIN_PASSWORD: usize = 10;

// ---------------------------------------------------------------------------
// Scope e ruoli
// ---------------------------------------------------------------------------

/// Tutte le azioni autorizzabili, con la descrizione mostrata nel pannello.
pub const SCOPES: &[(&str, &str)] = &[
    ("metrics:read", "Vedere le statistiche di traffico"),
    ("domains:read", "Vedere i domini"),
    (
        "domains:write",
        "Aggiungere, controllare ed eliminare domini",
    ),
    ("buckets:read", "Vedere i bucket"),
    (
        "buckets:write",
        "Aggiungere, controllare ed eliminare bucket",
    ),
    ("routes:read", "Vedere gli instradamenti e provare i file"),
    ("routes:write", "Creare ed eliminare instradamenti"),
    ("settings:write", "Modificare le impostazioni del nodo"),
    (
        "users:manage",
        "Gestire utenti, sicurezza e registro attività",
    ),
    (
        "notifications:manage",
        "Configurare le notifiche email e Telegram",
    ),
];

pub const MANAGE: &str = "users:manage";

pub fn is_scope(s: &str) -> bool {
    SCOPES.iter().any(|(k, _)| *k == s)
}

/// Ruoli predefiniti; "custom" usa la lista di scope dell'utente.
pub fn role_scopes(role: &str) -> Option<Vec<&'static str>> {
    match role {
        "admin" => Some(SCOPES.iter().map(|(k, _)| *k).collect()),
        "operator" => Some(vec![
            "metrics:read",
            "domains:write",
            "buckets:write",
            "routes:write",
        ]),
        "viewer" => Some(vec![
            "metrics:read",
            "domains:read",
            "buckets:read",
            "routes:read",
        ]),
        _ => None,
    }
}

pub fn is_role(r: &str) -> bool {
    r == "custom" || role_scopes(r).is_some()
}

/// Aggiunge le implicazioni: `x:write` include `x:read`.
fn close(mut set: BTreeSet<String>) -> BTreeSet<String> {
    let writes: Vec<String> = set
        .iter()
        .filter(|s| s.ends_with(":write"))
        .cloned()
        .collect();
    for w in writes {
        // solo gli scope che esistono nel catalogo (settings ha solo "write")
        let read = w.replace(":write", ":read");
        if is_scope(&read) {
            set.insert(read);
        }
    }
    set
}

// ---------------------------------------------------------------------------
// Modello
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Totp {
    /// segreto base32; con `enabled: false` è in attesa di conferma
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default)]
    pub enabled: bool,
    /// ultimo passo TOTP accettato: impedisce di riusare lo stesso codice
    #[serde(default)]
    pub last_step: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    /// usata solo con il ruolo "custom"
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub must_change_password: bool,
    #[serde(default)]
    pub totp: Totp,
    #[serde(default)]
    pub recovery_hashes: Vec<String>,
    pub created_at: String,
    #[serde(default)]
    pub last_login_at: Option<String>,
}

impl User {
    pub fn effective_scopes(&self) -> BTreeSet<String> {
        let base: BTreeSet<String> = match role_scopes(&self.role) {
            Some(v) => v.into_iter().map(str::to_owned).collect(),
            None => self
                .scopes
                .iter()
                .filter(|s| is_scope(s))
                .cloned()
                .collect(),
        };
        close(base)
    }

    pub fn can(&self, scope: &str) -> bool {
        self.effective_scopes().contains(scope)
    }

    pub fn is_manager(&self) -> bool {
        !self.disabled && self.can(MANAGE)
    }
}

/// Criterio del secondo fattore: nessuno, tutti, oppure chi gestisce gli utenti.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub require_2fa: String,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            require_2fa: "off".into(),
        }
    }
}

impl Policy {
    pub fn is_valid(v: &str) -> bool {
        matches!(v, "off" | "all" | "managers")
    }
    pub fn requires(&self, u: &User) -> bool {
        match self.require_2fa.as_str() {
            "all" => true,
            "managers" => u.can(MANAGE),
            _ => false,
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Data {
    #[serde(default)]
    users: Vec<User>,
    #[serde(default)]
    policy: Policy,
}

// ---------------------------------------------------------------------------
// Password
// ---------------------------------------------------------------------------

pub fn hash_password(pw: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(pw.as_bytes(), &SaltString::generate(&mut OsRng))
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub fn verify_password(pw: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

pub fn check_password(pw: &str) -> Result<(), String> {
    let n = pw.chars().count();
    if !(MIN_PASSWORD..=256).contains(&n) {
        return Err(format!(
            "la password deve avere almeno {MIN_PASSWORD} caratteri"
        ));
    }
    Ok(())
}

pub fn check_username(name: &str) -> Result<String, String> {
    let n = name.trim();
    let len = n.chars().count();
    if !(3..=64).contains(&len) || n.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("il nome utente deve avere da 3 a 64 caratteri, senza spazi".into());
    }
    Ok(n.to_owned())
}

/// Password temporanea leggibile (senza caratteri ambigui).
pub fn temp_password() -> String {
    const A: &[u8] = b"abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut raw = [0u8; 16];
    OsRng.fill_bytes(&mut raw);
    let s: String = raw
        .iter()
        .map(|b| A[*b as usize % A.len()] as char)
        .collect();
    format!("{}-{}-{}", &s[..5], &s[5..10], &s[10..])
}

pub fn new_id() -> String {
    let mut raw = [0u8; 12];
    OsRng.fill_bytes(&mut raw);
    hex::encode(raw)
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// ---------------------------------------------------------------------------
// Archivio
// ---------------------------------------------------------------------------

pub struct Store {
    path: PathBuf,
    data: Mutex<Data>,
}

/// Vecchio formato a un solo amministratore, da migrare.
#[derive(Deserialize)]
struct LegacyAccount {
    username: String,
    password_hash: String,
}

impl Store {
    /// Carica `users.json`. Se non esiste ma c'è il vecchio `admin.json`, il suo
    /// utente diventa Amministratore e il file viene rinominato `.migrated`.
    pub fn load(state_dir: &Path) -> Store {
        let path = state_dir.join("users.json");
        let mut data: Data = std::fs::read(&path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        let legacy = state_dir.join("admin.json");
        if data.users.is_empty() && legacy.exists() {
            if let Some(acc) = std::fs::read(&legacy)
                .ok()
                .and_then(|r| serde_json::from_slice::<LegacyAccount>(&r).ok())
            {
                data.users.push(User {
                    id: new_id(),
                    username: acc.username,
                    password_hash: acc.password_hash,
                    role: "admin".into(),
                    scopes: vec![],
                    disabled: false,
                    must_change_password: false,
                    totp: Totp::default(),
                    recovery_hashes: vec![],
                    created_at: now_iso(),
                    last_login_at: None,
                });
                let store = Store {
                    path: path.clone(),
                    data: Mutex::new(data),
                };
                if store.save().is_ok() {
                    let _ = std::fs::rename(&legacy, state_dir.join("admin.json.migrated"));
                    tracing::info!("utente amministratore migrato da admin.json a users.json");
                }
                return store;
            }
        }
        Store {
            path,
            data: Mutex::new(data),
        }
    }

    fn save(&self) -> std::io::Result<()> {
        let bytes = {
            let d = self.data.lock().unwrap();
            serde_json::to_vec_pretty(&*d).map_err(std::io::Error::other)?
        };
        crate::admin::write_atomic(&self.path, &bytes, true)
    }

    pub fn is_empty(&self) -> bool {
        self.data.lock().unwrap().users.is_empty()
    }

    pub fn list(&self) -> Vec<User> {
        self.data.lock().unwrap().users.clone()
    }

    pub fn get(&self, id: &str) -> Option<User> {
        self.data
            .lock()
            .unwrap()
            .users
            .iter()
            .find(|u| u.id == id)
            .cloned()
    }

    pub fn find_by_name(&self, name: &str) -> Option<User> {
        let n = name.trim().to_lowercase();
        self.data
            .lock()
            .unwrap()
            .users
            .iter()
            .find(|u| u.username.to_lowercase() == n)
            .cloned()
    }

    pub fn policy(&self) -> Policy {
        self.data.lock().unwrap().policy.clone()
    }

    pub fn set_policy(&self, p: Policy) -> Result<(), String> {
        self.data.lock().unwrap().policy = p;
        self.save().map_err(|e| format!("salvataggio: {e}"))
    }

    pub fn insert(&self, u: User) -> Result<User, String> {
        {
            let mut d = self.data.lock().unwrap();
            if d.users
                .iter()
                .any(|x| x.username.to_lowercase() == u.username.to_lowercase())
            {
                return Err(format!("esiste già un utente '{}'", u.username));
            }
            d.users.push(u.clone());
        }
        self.save().map_err(|e| format!("salvataggio: {e}"))?;
        Ok(u)
    }

    /// Modifica un utente e salva; annulla tutto se dopo la modifica non resterebbe
    /// nessun utente attivo con lo scope `users:manage`.
    pub fn update<F>(&self, id: &str, f: F) -> Result<User, String>
    where
        F: FnOnce(&mut User) -> Result<(), String>,
    {
        let updated = {
            let mut d = self.data.lock().unwrap();
            let idx = d
                .users
                .iter()
                .position(|u| u.id == id)
                .ok_or("utente non trovato")?;
            let mut copy = d.users[idx].clone();
            f(&mut copy)?;
            if copy.username.to_lowercase() != d.users[idx].username.to_lowercase()
                && d.users.iter().any(|x| {
                    x.id != id && x.username.to_lowercase() == copy.username.to_lowercase()
                })
            {
                return Err(format!("esiste già un utente '{}'", copy.username));
            }
            let managers = d
                .users
                .iter()
                .enumerate()
                .filter(|(i, u)| {
                    if *i == idx {
                        copy.is_manager()
                    } else {
                        u.is_manager()
                    }
                })
                .count();
            if managers == 0 {
                return Err(
                    "deve restare almeno un utente attivo che può gestire gli utenti".into(),
                );
            }
            d.users[idx] = copy.clone();
            copy
        };
        self.save().map_err(|e| format!("salvataggio: {e}"))?;
        Ok(updated)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        {
            let mut d = self.data.lock().unwrap();
            let idx = d
                .users
                .iter()
                .position(|u| u.id == id)
                .ok_or("utente non trovato")?;
            let remaining = d
                .users
                .iter()
                .enumerate()
                .filter(|(i, u)| *i != idx && u.is_manager())
                .count();
            if remaining == 0 {
                return Err(
                    "deve restare almeno un utente attivo che può gestire gli utenti".into(),
                );
            }
            d.users.remove(idx);
        }
        self.save().map_err(|e| format!("salvataggio: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(name: &str, role: &str) -> User {
        User {
            id: new_id(),
            username: name.into(),
            password_hash: hash_password("una-password-lunga").unwrap(),
            role: role.into(),
            scopes: vec![],
            disabled: false,
            must_change_password: false,
            totp: Totp::default(),
            recovery_hashes: vec![],
            created_at: now_iso(),
            last_login_at: None,
        }
    }

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::load(dir.path());
        (dir, s)
    }

    #[test]
    fn roles_expand_and_write_implies_read() {
        let admin = user("a", "admin");
        assert!(SCOPES.iter().all(|(k, _)| admin.can(k)));
        let op = user("o", "operator");
        assert!(op.can("domains:write") && op.can("domains:read"));
        assert!(!op.can("settings:write") && !op.can(MANAGE));
        let viewer = user("v", "viewer");
        assert!(viewer.can("buckets:read") && !viewer.can("buckets:write"));
        let mut custom = user("c", "custom");
        custom.scopes = vec!["routes:write".into(), "inventato:x".into()];
        assert!(custom.can("routes:read") && custom.can("routes:write"));
        assert!(
            !custom.can("inventato:x"),
            "gli scope sconosciuti si ignorano"
        );
        assert!(is_role("custom") && is_role("viewer") && !is_role("root"));
    }

    #[test]
    fn last_manager_is_protected() {
        let (_d, s) = store();
        let a = s.insert(user("alice", "admin")).unwrap();
        let b = s.insert(user("bob", "operator")).unwrap();
        // l'unico gestore non si elimina, non si disabilita, non si declassa
        assert!(s.delete(&a.id).is_err());
        assert!(s
            .update(&a.id, |u| {
                u.disabled = true;
                Ok(())
            })
            .is_err());
        assert!(s
            .update(&a.id, |u| {
                u.role = "viewer".into();
                Ok(())
            })
            .is_err());
        // con un secondo gestore si può
        s.update(&b.id, |u| {
            u.role = "admin".into();
            Ok(())
        })
        .unwrap();
        s.update(&a.id, |u| {
            u.disabled = true;
            Ok(())
        })
        .unwrap();
        // ora bob è l'ultimo
        assert!(s.delete(&b.id).is_err());
        s.delete(&a.id).unwrap();
        assert_eq!(s.list().len(), 1);
    }

    #[test]
    fn usernames_are_unique_case_insensitive() {
        let (_d, s) = store();
        s.insert(user("Alice", "admin")).unwrap();
        assert!(s.insert(user("alice", "viewer")).is_err());
        assert!(s.find_by_name(" ALICE ").is_some());
    }

    #[test]
    fn persistence_and_file_mode() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = Store::load(dir.path());
            s.insert(user("alice", "admin")).unwrap();
            s.set_policy(Policy {
                require_2fa: "all".into(),
            })
            .unwrap();
        }
        let s = Store::load(dir.path());
        assert_eq!(s.list().len(), 1);
        assert_eq!(s.policy().require_2fa, "all");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("users.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn migrates_legacy_admin_json() {
        let dir = tempfile::tempdir().unwrap();
        let hash = hash_password("una-password-lunga").unwrap();
        std::fs::write(
            dir.path().join("admin.json"),
            serde_json::json!({"username": "admin", "password_hash": hash}).to_string(),
        )
        .unwrap();
        let s = Store::load(dir.path());
        let u = s.find_by_name("admin").unwrap();
        assert_eq!(u.role, "admin");
        assert!(verify_password("una-password-lunga", &u.password_hash));
        assert!(!dir.path().join("admin.json").exists());
        assert!(dir.path().join("admin.json.migrated").exists());
        // un secondo avvio non migra di nuovo
        assert_eq!(Store::load(dir.path()).list().len(), 1);
    }

    #[test]
    fn policy_applies_to_the_right_users() {
        let admin = user("a", "admin");
        let viewer = user("v", "viewer");
        let p = |v: &str| Policy {
            require_2fa: v.into(),
        };
        assert!(!p("off").requires(&admin));
        assert!(p("all").requires(&viewer));
        assert!(p("managers").requires(&admin) && !p("managers").requires(&viewer));
        assert!(Policy::is_valid("managers") && !Policy::is_valid("boh"));
    }

    #[test]
    fn validation_helpers() {
        assert!(check_username("ab").is_err());
        assert!(check_username("con spazio").is_err());
        assert_eq!(check_username("  marco ").unwrap(), "marco");
        assert!(check_password("corta").is_err());
        assert!(check_password("una-password-lunga").is_ok());
        assert_ne!(temp_password(), temp_password());
        assert!(check_password(&temp_password()).is_ok());
    }
}
