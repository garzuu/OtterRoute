//! Certificati automatici (ACME, sfida HTTP-01). Per ogni dominio verificato il
//! nodo ottiene un certificato, lo salva in `state/certs/<host>/` e lo rinnova
//! 30 giorni prima della scadenza. La sfida passa dalla porta 80 del nodo.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use instant_acme::{
    Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt,
    NewAccount, NewOrder, OrderStatus, RetryPolicy,
};
use sha2::{Digest, Sha256};
use tokio::sync::Notify;

use crate::panel::{self, AcmeSettings};
use crate::tls::{self, Tls};

/// Dopo un errore non si riprova prima di questo tempo (i limiti delle CA sono severi).
const RETRY_AFTER_SECS: u64 = 3600;
const PASS_EVERY: Duration = Duration::from_secs(600);
const ISSUE_TIMEOUT: Duration = Duration::from_secs(240);

pub struct Acme {
    state_dir: PathBuf,
    tls: Arc<Tls>,
    /// indirizzo della directory ACME, se diverso da Let's Encrypt (per le prove con Pebble)
    directory: Option<String>,
    /// certificato radice da fidarsi per quella directory
    ca_root: Option<PathBuf>,
    issuing: Mutex<HashSet<String>>,
    kick: Notify,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub fn valid_email(e: &str) -> bool {
    let e = e.trim();
    e.is_empty()
        || (e.len() <= 254
            && !e.chars().any(|c| c.is_control() || c.is_whitespace())
            && e.split_once('@').is_some_and(|(u, d)| {
                !u.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')
            }))
}

/// Un nome che una CA può certificare: un DNS, non un IP né un dominio locale.
pub fn certifiable(host: &str) -> bool {
    tls::safe_host(host)
        && host.contains('.')
        && !crate::dns::is_local_name(host)
        && host.parse::<std::net::IpAddr>().is_err()
}

impl Acme {
    pub fn new(
        state_dir: PathBuf,
        tls: Arc<Tls>,
        directory: Option<String>,
        ca_root: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state_dir,
            tls,
            directory: directory.filter(|d| !d.is_empty()),
            ca_root,
            issuing: Mutex::new(HashSet::new()),
            kick: Notify::new(),
        })
    }

    /// Fa ripartire subito un giro di controllo (dopo una modifica dal pannello).
    pub fn kick(&self) {
        self.kick.notify_one();
    }

    pub fn is_issuing(&self, host: &str) -> bool {
        self.issuing.lock().unwrap().contains(host)
    }

    fn directory_url(&self, s: &AcmeSettings) -> String {
        match &self.directory {
            Some(d) => d.clone(),
            None if s.staging => LetsEncrypt::Staging.url().to_owned(),
            None => LetsEncrypt::Production.url().to_owned(),
        }
    }

    fn account_path(&self, directory: &str) -> PathBuf {
        let h = hex::encode(Sha256::digest(directory.as_bytes()));
        self.state_dir
            .join("secrets")
            .join(format!("_acme-{}.json", &h[..16]))
    }

    fn builder(&self) -> Result<instant_acme::AccountBuilder, String> {
        match &self.ca_root {
            Some(p) => Account::builder_with_root(p),
            None => Account::builder(),
        }
        .map_err(|e| format!("client ACME: {e}"))
    }

    /// L'account presso la CA: si crea al primo uso e si conserva (una CA diversa, un account diverso).
    async fn account(&self, s: &AcmeSettings) -> Result<Account, String> {
        let dir = self.directory_url(s);
        let path = self.account_path(&dir);
        if let Some(cred) = std::fs::read(&path)
            .ok()
            .and_then(|r| serde_json::from_slice::<AccountCredentials>(&r).ok())
        {
            return self
                .builder()?
                .from_credentials(cred)
                .await
                .map_err(|e| format!("account ACME: {e}"));
        }
        let contact = format!("mailto:{}", s.email.trim());
        let contacts: Vec<&str> = if s.email.trim().is_empty() {
            vec![]
        } else {
            vec![contact.as_str()]
        };
        let (account, cred) = self
            .builder()?
            .create(
                &NewAccount {
                    contact: &contacts,
                    terms_of_service_agreed: true,
                    only_return_existing: false,
                },
                dir,
                None,
            )
            .await
            .map_err(|e| format!("creazione dell'account ACME: {e}"))?;
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let raw = serde_json::to_vec_pretty(&cred).map_err(|e| e.to_string())?;
        crate::admin::write_atomic(&path, &raw, true)
            .map_err(|e| format!("salvataggio dell'account: {e}"))?;
        Ok(account)
    }

    async fn order(&self, host: &str, s: &AcmeSettings) -> Result<(String, String), String> {
        let account = self.account(s).await?;
        let ids = [Identifier::Dns(host.to_owned())];
        let mut order = account
            .new_order(&NewOrder::new(&ids))
            .await
            .map_err(|e| format!("nuovo ordine: {e}"))?;
        let mut tokens: Vec<String> = Vec::new();
        let result = async {
            let mut authzs = order.authorizations();
            while let Some(r) = authzs.next().await {
                let mut authz = r.map_err(|e| format!("autorizzazione: {e}"))?;
                match authz.status {
                    AuthorizationStatus::Pending => {}
                    AuthorizationStatus::Valid => continue,
                    other => return Err(format!("autorizzazione in stato {other:?}")),
                }
                let mut ch = authz
                    .challenge(ChallengeType::Http01)
                    .ok_or("la CA non offre la sfida HTTP-01")?;
                let token = ch.token.clone();
                self.tls
                    .challenges
                    .lock()
                    .unwrap()
                    .insert(token.clone(), ch.key_authorization().as_str().to_owned());
                tokens.push(token);
                ch.set_ready().await.map_err(|e| format!("avvio della sfida: {e}"))?;
            }
            let status = order
                .poll_ready(&RetryPolicy::default().timeout(Duration::from_secs(90)))
                .await
                .map_err(|e| format!("attesa della verifica: {e}"))?;
            if status != OrderStatus::Ready {
                return Err(format!(
                    "la CA non ha convalidato il dominio (stato {status:?}): controlla che {host} arrivi a questo nodo sulla porta 80 da Internet"
                ));
            }
            let key = order.finalize().await.map_err(|e| format!("finalizzazione: {e}"))?;
            let chain = order
                .poll_certificate(&RetryPolicy::default().timeout(Duration::from_secs(60)))
                .await
                .map_err(|e| format!("download del certificato: {e}"))?;
            Ok((chain, key))
        }
        .await;
        for t in tokens {
            self.tls.challenges.lock().unwrap().remove(&t);
        }
        result
    }

    /// Ottiene e installa il certificato di un dominio. L'esito resta su disco.
    pub async fn issue(self: &Arc<Self>, host: &str) -> Result<u64, String> {
        if !certifiable(host) {
            return Err("questo nome non può avere un certificato pubblico".into());
        }
        {
            let mut set = self.issuing.lock().unwrap();
            if !set.insert(host.to_owned()) {
                return Err("è già in corso un'emissione per questo dominio".into());
            }
        }
        let settings = panel::load(&self.state_dir)
            .map(|p| p.settings.acme)
            .unwrap_or_default();
        let res = match tokio::time::timeout(ISSUE_TIMEOUT, self.order(host, &settings)).await {
            Ok(r) => r,
            Err(_) => Err("tempo scaduto: la CA non ha risposto in 4 minuti".into()),
        };
        let out = match res {
            Ok((chain, key)) => tls::save_cert(&self.state_dir, host, &chain, &key, now_secs())
                .and_then(|meta| {
                    tls::load_host(&self.state_dir, host, &self.tls.store).map(|_| meta.not_after)
                }),
            Err(e) => Err(e),
        };
        match &out {
            Ok(na) => tracing::info!(host, scade = na, "certificato emesso"),
            Err(e) => {
                tracing::warn!(host, error = %e, "emissione del certificato non riuscita");
                tls::save_error(&self.state_dir, host, e, now_secs());
            }
        }
        self.issuing.lock().unwrap().remove(host);
        out
    }

    /// Un giro: emette o rinnova quello che serve.
    async fn pass(self: &Arc<Self>) {
        let Ok(p) = panel::load(&self.state_dir) else {
            return;
        };
        if !p.settings.acme.enabled {
            return;
        }
        let now = now_secs();
        for d in p
            .domains
            .iter()
            .filter(|d| d.verified && certifiable(&d.host))
        {
            let mut info = tls::read_info(&self.state_dir, &d.host);
            let mut loaded = self.tls.store.has(&d.host);
            if !loaded
                && info.not_after.is_some()
                && tls::load_host(&self.state_dir, &d.host, &self.tls.store).is_ok()
            {
                loaded = true;
                info = tls::read_info(&self.state_dir, &d.host);
            }
            if !tls::needs_issue(&info, loaded, now) {
                continue;
            }
            if info
                .error
                .as_ref()
                .is_some_and(|e| now.saturating_sub(e.at) < RETRY_AFTER_SECS)
            {
                continue;
            }
            let _ = self.issue(&d.host).await;
        }
    }

    pub async fn run(self: Arc<Self>) {
        let mut tick = tokio::time::interval(PASS_EVERY);
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                _ = self.kick.notified() => {}
            }
            self.pass().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_and_names() {
        assert!(valid_email("") && valid_email("a@example.com") && valid_email("  "));
        assert!(
            !valid_email("no-at")
                && !valid_email("a@b")
                && !valid_email("a b@example.com")
                && !valid_email("@example.com")
        );
        assert!(!valid_email("a@example.com\nBcc: x@y.it"));
        assert!(certifiable("cdn.example.com"));
        assert!(
            !certifiable("img.localhost")
                && !certifiable("localhost")
                && !certifiable("10.0.0.1")
                && !certifiable("nodots")
                && !certifiable("../x.it")
        );
    }

    #[test]
    fn account_file_depends_on_the_directory() {
        let a = Acme::new(PathBuf::from("/s"), Tls::new(443), None, None);
        let p1 = a.account_path("https://acme-staging-v02.api.letsencrypt.org/directory");
        let p2 = a.account_path("https://acme-v02.api.letsencrypt.org/directory");
        assert_ne!(p1, p2);
        assert!(
            p1.starts_with("/s/secrets")
                && p1
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("_acme-")
        );
    }

    #[test]
    fn directory_selection() {
        let s = AcmeSettings {
            enabled: true,
            email: String::new(),
            staging: true,
        };
        let a = Acme::new(PathBuf::from("/s"), Tls::new(443), None, None);
        assert!(a.directory_url(&s).contains("staging"));
        assert!(!a
            .directory_url(&AcmeSettings {
                staging: false,
                ..s.clone()
            })
            .contains("staging"));
        let b = Acme::new(
            PathBuf::from("/s"),
            Tls::new(443),
            Some("https://localhost:14000/dir".into()),
            None,
        );
        assert_eq!(b.directory_url(&s), "https://localhost:14000/dir");
    }
}
