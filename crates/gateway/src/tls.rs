//! HTTPS: archivio dei certificati (uno per dominio, scelto con l'SNI), listener
//! TLS e stato delle sfide ACME. I certificati stanno in `state/certs/<host>/`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::body;
use hyper::Response;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub fn certs_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("certs")
}

fn host_dir(state_dir: &Path, host: &str) -> PathBuf {
    certs_dir(state_dir).join(host)
}

/// Un nome DNS valido come cartella: niente separatori né caratteri strani.
pub fn safe_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.starts_with('.')
        && !host.contains("..")
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Metadati di un certificato, salvati accanto ai file PEM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CertMeta {
    /// scadenza (secondi Unix)
    pub not_after: u64,
    pub issued_at: u64,
}

/// Ultimo tentativo fallito di emissione per un dominio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CertError {
    pub at: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CertInfo {
    pub host: String,
    pub not_after: Option<u64>,
    pub issued_at: Option<u64>,
    pub error: Option<CertError>,
}

/// Certificati caricati in memoria, scelti in base al nome richiesto dal client (SNI).
#[derive(Default)]
pub struct CertStore {
    map: RwLock<HashMap<String, Arc<CertifiedKey>>>,
}

impl std::fmt::Debug for CertStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CertStore").finish_non_exhaustive()
    }
}

impl ResolvesServerCert for CertStore {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let name = hello.server_name()?.to_ascii_lowercase();
        self.map.read().unwrap().get(&name).cloned()
    }
}

impl CertStore {
    pub fn set(&self, host: &str, key: CertifiedKey) {
        self.map
            .write()
            .unwrap()
            .insert(host.to_ascii_lowercase(), Arc::new(key));
    }
    pub fn remove(&self, host: &str) {
        self.map.write().unwrap().remove(&host.to_ascii_lowercase());
    }
    pub fn has(&self, host: &str) -> bool {
        self.map
            .read()
            .unwrap()
            .contains_key(&host.to_ascii_lowercase())
    }
}

/// Stato condiviso di HTTPS: sfide ACME in corso, certificati, domini con redirect.
pub struct Tls {
    /// token della sfida HTTP-01 → risposta attesa (key authorization)
    pub challenges: Mutex<HashMap<String, String>>,
    pub store: Arc<CertStore>,
    /// domini per cui l'HTTP risponde con un redirect a HTTPS
    pub redirect: RwLock<HashSet<String>>,
    /// porta HTTPS pubblica, per costruire l'indirizzo del redirect
    pub https_port: AtomicU16,
    /// il listener HTTPS è davvero in ascolto
    listening: std::sync::atomic::AtomicBool,
}

impl Tls {
    pub fn new(https_port: u16) -> Arc<Self> {
        Arc::new(Self {
            challenges: Mutex::new(HashMap::new()),
            store: Arc::new(CertStore::default()),
            redirect: RwLock::new(HashSet::new()),
            https_port: AtomicU16::new(https_port),
            listening: std::sync::atomic::AtomicBool::new(false),
        })
    }

    pub fn set_listening(&self, v: bool) {
        self.listening.store(v, Ordering::Relaxed);
    }

    pub fn is_listening(&self) -> bool {
        self.listening.load(Ordering::Relaxed)
    }

    /// Allinea redirect e porta a quanto salvato nel pannello. Un redirect si applica
    /// solo se il certificato del dominio è in memoria: altrimenti il sito si romperebbe.
    pub fn apply_panel(&self, p: &crate::panel::Panel) {
        self.https_port.store(p.settings.https(), Ordering::Relaxed);
        let hosts = p
            .domains
            .iter()
            .filter(|d| d.redirect_https && self.store.has(&d.host))
            .map(|d| d.host.clone());
        self.set_redirects(hosts);
    }

    pub fn challenge_response(&self, token: &str) -> Option<String> {
        self.challenges.lock().unwrap().get(token).cloned()
    }

    pub fn wants_redirect(&self, host: &str) -> bool {
        self.redirect.read().unwrap().contains(host)
    }

    pub fn set_redirects(&self, hosts: impl IntoIterator<Item = String>) {
        *self.redirect.write().unwrap() = hosts.into_iter().collect();
    }

    /// Indirizzo HTTPS equivalente a una richiesta HTTP.
    pub fn https_location(&self, host: &str, path_and_query: &str) -> String {
        let port = self.https_port.load(Ordering::Relaxed);
        if port == 443 {
            format!("https://{host}{path_and_query}")
        } else {
            format!("https://{host}:{port}{path_and_query}")
        }
    }
}

// --- lettura e scrittura su disco --------------------------------------------

/// Legge catena e chiave PEM e ne ricava il certificato pronto e la scadenza.
pub fn parse_pem(chain_pem: &[u8], key_pem: &[u8]) -> Result<(CertifiedKey, u64), String> {
    let chain: Vec<_> = rustls_pemfile::certs(&mut &chain_pem[..])
        .collect::<Result<_, _>>()
        .map_err(|e| format!("catena PEM non valida: {e}"))?;
    let first = chain.first().ok_or("la catena non contiene certificati")?;
    let key = rustls_pemfile::private_key(&mut &key_pem[..])
        .map_err(|e| format!("chiave PEM non valida: {e}"))?
        .ok_or("la chiave privata manca")?;
    let (_, cert) = x509_parser::parse_x509_certificate(first.as_ref())
        .map_err(|e| format!("certificato non valido: {e}"))?;
    let not_after = u64::try_from(cert.validity().not_after.timestamp()).unwrap_or(0);
    let signing = rustls::crypto::ring::sign::any_supported_type(&key)
        .map_err(|e| format!("chiave non supportata: {e}"))?;
    Ok((CertifiedKey::new(chain, signing), not_after))
}

fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    crate::admin::write_atomic(path, data, true)
}

pub fn save_cert(
    state_dir: &Path,
    host: &str,
    chain_pem: &str,
    key_pem: &str,
    now: u64,
) -> Result<CertMeta, String> {
    if !safe_host(host) {
        return Err("nome di dominio non valido".into());
    }
    let (_, not_after) = parse_pem(chain_pem.as_bytes(), key_pem.as_bytes())?;
    let dir = host_dir(state_dir, host);
    std::fs::create_dir_all(&dir).map_err(|e| format!("creazione cartella: {e}"))?;
    write_private(&dir.join("privkey.pem"), key_pem.as_bytes())
        .map_err(|e| format!("scrittura chiave: {e}"))?;
    crate::admin::write_atomic(&dir.join("fullchain.pem"), chain_pem.as_bytes(), false)
        .map_err(|e| format!("scrittura catena: {e}"))?;
    let meta = CertMeta {
        not_after,
        issued_at: now,
    };
    let raw = serde_json::to_vec_pretty(&meta).map_err(|e| e.to_string())?;
    crate::admin::write_atomic(&dir.join("meta.json"), &raw, false)
        .map_err(|e| format!("scrittura metadati: {e}"))?;
    let _ = std::fs::remove_file(dir.join("error.json"));
    Ok(meta)
}

pub fn save_error(state_dir: &Path, host: &str, message: &str, now: u64) {
    if !safe_host(host) {
        return;
    }
    let dir = host_dir(state_dir, host);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let e = CertError {
        at: now,
        message: message.chars().take(500).collect(),
    };
    if let Ok(raw) = serde_json::to_vec_pretty(&e) {
        let _ = crate::admin::write_atomic(&dir.join("error.json"), &raw, false);
    }
}

pub fn read_info(state_dir: &Path, host: &str) -> CertInfo {
    let dir = host_dir(state_dir, host);
    let meta: Option<CertMeta> = std::fs::read(dir.join("meta.json"))
        .ok()
        .and_then(|r| serde_json::from_slice(&r).ok());
    let error: Option<CertError> = std::fs::read(dir.join("error.json"))
        .ok()
        .and_then(|r| serde_json::from_slice(&r).ok());
    CertInfo {
        host: host.to_owned(),
        not_after: meta.as_ref().map(|m| m.not_after),
        issued_at: meta.as_ref().map(|m| m.issued_at),
        error,
    }
}

/// Carica in memoria il certificato di un dominio, se c'è ed è leggibile.
pub fn load_host(state_dir: &Path, host: &str, store: &CertStore) -> Result<u64, String> {
    let dir = host_dir(state_dir, host);
    let chain = std::fs::read(dir.join("fullchain.pem")).map_err(|e| format!("catena: {e}"))?;
    let key = std::fs::read(dir.join("privkey.pem")).map_err(|e| format!("chiave: {e}"))?;
    let (ck, not_after) = parse_pem(&chain, &key)?;
    store.set(host, ck);
    Ok(not_after)
}

/// All'avvio: tutti i certificati salvati.
pub fn load_all(state_dir: &Path, store: &CertStore) -> usize {
    let Ok(rd) = std::fs::read_dir(certs_dir(state_dir)) else {
        return 0;
    };
    let mut n = 0;
    for e in rd.flatten() {
        let host = e.file_name().to_string_lossy().into_owned();
        if !safe_host(&host) {
            continue;
        }
        match load_host(state_dir, &host, store) {
            Ok(_) => n += 1,
            Err(e) if std::fs::metadata(e_path(state_dir, &host)).is_ok() => {
                tracing::warn!(host = %host, error = %e, "certificato non caricabile");
            }
            Err(_) => {}
        }
    }
    n
}

fn e_path(state_dir: &Path, host: &str) -> PathBuf {
    host_dir(state_dir, host).join("fullchain.pem")
}

// --- server TLS -----------------------------------------------------------------

pub fn acceptor(store: Arc<CertStore>) -> Result<TlsAcceptor, rustls::Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_cert_resolver(store);
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(TlsAcceptor::from(Arc::new(cfg)))
}

/// Come `serve` di main, ma con TLS: un handshake che non arriva in 10 secondi
/// o senza certificato per il nome richiesto si chiude senza toccare gli altri.
pub async fn serve<F, Fut>(listener: TcpListener, acceptor: TlsAcceptor, f: F)
where
    F: Fn(http::Request<hyper::body::Incoming>) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = Result<Response<body::Body>, std::convert::Infallible>>
        + Send
        + 'static,
{
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "accept https");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let (acceptor, f) = (acceptor.clone(), f.clone());
        tokio::spawn(async move {
            let tls = match tokio::time::timeout(HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    tracing::debug!(error = %e, "handshake TLS non riuscito");
                    return;
                }
                Err(_) => return,
            };
            let svc = service_fn(f);
            if let Err(e) = auto::Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(tls), svc)
                .await
            {
                tracing::debug!(error = %e, "connessione https chiusa");
            }
        });
    }
}

// --- rinnovo ---------------------------------------------------------------------

/// Quanto prima della scadenza si rinnova, al massimo.
pub const RENEW_BEFORE_SECS: u64 = 30 * 24 * 3600;
/// Da quanto prima della scadenza lo stato diventa "in scadenza", al massimo.
const EXPIRING_SECS: u64 = 14 * 24 * 3600;

/// Le finestre si accorciano per i certificati di breve durata: non si può rinnovare
/// a 30 giorni dalla scadenza un certificato che ne dura 6, o si rinnoverebbe a ogni giro.
fn window(info: &CertInfo, max: u64) -> u64 {
    match (info.issued_at, info.not_after) {
        (Some(from), Some(to)) if to > from => max.min((to - from) / 3),
        _ => max,
    }
}

pub fn needs_issue(info: &CertInfo, has_loaded: bool, now: u64) -> bool {
    match info.not_after {
        None => true,
        Some(na) => !has_loaded || na.saturating_sub(now) < window(info, RENEW_BEFORE_SECS),
    }
}

/// Stato per la tabella dei domini.
pub fn status(info: &CertInfo, now: u64) -> &'static str {
    match info.not_after {
        None if info.error.is_some() => "error",
        None => "missing",
        Some(na) if na <= now => "expired",
        Some(na) if na - now < window(info, EXPIRING_SECS) => "expiring",
        Some(_) => "valid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub fn self_signed(host: &str, days: i64) -> (String, String) {
        let mut params = rcgen::CertificateParams::new(vec![host.to_owned()]).unwrap();
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(1);
        params.not_after = now + time::Duration::days(days);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        (cert.pem(), key.serialize_pem())
    }

    #[test]
    fn parse_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let (chain, key) = self_signed("a.example.com", 90);
        let meta = save_cert(dir.path(), "a.example.com", &chain, &key, 1000).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(meta.not_after > now + 80 * 86400 && meta.not_after < now + 100 * 86400);
        let info = read_info(dir.path(), "a.example.com");
        assert_eq!(info.not_after, Some(meta.not_after));
        assert_eq!(status(&info, now), "valid");
        assert!(!needs_issue(&info, true, now));
        assert!(
            needs_issue(&info, false, now),
            "non ancora in memoria: da ricaricare"
        );
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join("certs/a.example.com/privkey.pem"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "la chiave privata è riservata");
        let store = CertStore::default();
        assert_eq!(load_all(dir.path(), &store), 1);
        assert!(store.has("A.Example.com"));
        assert!(!store.has("b.example.com"));
    }

    #[test]
    fn expiry_windows_and_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let (chain, key) = self_signed("b.example.com", 10);
        save_cert(dir.path(), "b.example.com", &chain, &key, 0).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let info = read_info(dir.path(), "b.example.com");
        assert_eq!(status(&info, now), "expiring");
        assert!(
            needs_issue(&info, true, now),
            "meno di 30 giorni: si rinnova"
        );
        assert_eq!(status(&info, now + 20 * 86400), "expired");
        assert_eq!(
            status(&read_info(dir.path(), "nuovo.example.com"), now),
            "missing"
        );
        save_error(dir.path(), "nuovo.example.com", "boom", now);
        assert_eq!(
            status(&read_info(dir.path(), "nuovo.example.com"), now),
            "error"
        );
        // input non validi
        assert!(
            save_cert(dir.path(), "../evil", &chain, &key, 0).is_err(),
            "niente path traversal"
        );
        assert!(save_cert(dir.path(), "c.example.com", "non è pem", &key, 0).is_err());
        assert!(save_cert(dir.path(), "c.example.com", &chain, "non è pem", 0).is_err());
        assert!(
            !safe_host("a/b") && !safe_host("") && !safe_host(".x") && safe_host("a-b.example.com")
        );
    }

    #[test]
    fn short_lived_certificates_do_not_renew_in_a_loop() {
        let info = |lifetime_days: u64, left_days: u64| CertInfo {
            host: "a.example.com".into(),
            issued_at: Some(1_000_000),
            not_after: Some(1_000_000 + lifetime_days * 86400),
            error: None,
        };
        let at = |days_since_issue: u64| 1_000_000 + days_since_issue * 86400;
        // 90 giorni: come prima (rinnovo a 30 giorni dalla scadenza, "in scadenza" a 14)
        assert!(
            !needs_issue(&info(90, 0), true, at(59)) && needs_issue(&info(90, 0), true, at(61))
        );
        assert_eq!(status(&info(90, 0), at(60)), "valid");
        assert_eq!(status(&info(90, 0), at(77)), "expiring");
        // 6 giorni: si rinnova solo nell'ultimo terzo, e non è "in scadenza" appena emesso
        assert_eq!(status(&info(6, 0), at(0)), "valid");
        assert!(!needs_issue(&info(6, 0), true, at(1)));
        assert!(needs_issue(&info(6, 0), true, at(5)));
        assert_eq!(status(&info(6, 0), at(5)), "expiring");
        assert_eq!(status(&info(6, 0), at(7)), "expired");
    }

    #[test]
    fn challenges_and_redirects() {
        let t = Tls::new(8443);
        t.challenges
            .lock()
            .unwrap()
            .insert("tok".into(), "tok.thumb".into());
        assert_eq!(t.challenge_response("tok").as_deref(), Some("tok.thumb"));
        assert_eq!(t.challenge_response("altro"), None);
        t.set_redirects(["a.it".to_string()]);
        assert!(t.wants_redirect("a.it") && !t.wants_redirect("b.it"));
        assert_eq!(
            t.https_location("a.it", "/x?y=1"),
            "https://a.it:8443/x?y=1"
        );
        t.https_port.store(443, Ordering::Relaxed);
        assert_eq!(t.https_location("a.it", "/x"), "https://a.it/x");
    }

    fn client(
        root_pem: &str,
        sni: &str,
    ) -> (
        tokio_rustls::TlsConnector,
        rustls::pki_types::ServerName<'static>,
    ) {
        let mut roots = rustls::RootCertStore::empty();
        for c in rustls_pemfile::certs(&mut root_pem.as_bytes()) {
            roots.add(c.unwrap()).unwrap();
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        (
            tokio_rustls::TlsConnector::from(Arc::new(cfg)),
            rustls::pki_types::ServerName::try_from(sni.to_owned()).unwrap(),
        )
    }

    #[tokio::test]
    async fn serves_each_name_with_its_own_certificate() {
        let (chain_a, key_a) = self_signed("a.example.com", 30);
        let (chain_b, key_b) = self_signed("b.example.com", 30);
        let store = Arc::new(CertStore::default());
        store.set(
            "a.example.com",
            parse_pem(chain_a.as_bytes(), key_a.as_bytes()).unwrap().0,
        );
        store.set(
            "b.example.com",
            parse_pem(chain_b.as_bytes(), key_b.as_bytes()).unwrap().0,
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(serve(
            listener,
            acceptor(store).unwrap(),
            |req: http::Request<hyper::body::Incoming>| async move {
                let host = req
                    .headers()
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-")
                    .to_owned();
                Ok::<_, std::convert::Infallible>(Response::new(body::full(format!(
                    "ciao da {host}"
                ))))
            },
        ));

        for (sni, chain) in [("a.example.com", &chain_a), ("b.example.com", &chain_b)] {
            let (conn, name) = client(chain, sni);
            let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
            let mut tls = conn
                .connect(name, tcp)
                .await
                .expect("handshake con il certificato giusto");
            tls.write_all(
                format!("GET / HTTP/1.1\r\nHost: {sni}\r\nConnection: close\r\n\r\n").as_bytes(),
            )
            .await
            .unwrap();
            let mut out = String::new();
            tls.read_to_string(&mut out).await.ok();
            assert!(
                out.starts_with("HTTP/1.1 200") && out.contains(&format!("ciao da {sni}")),
                "{out}"
            );
        }
        // il certificato di A non vale per B
        let (conn, name) = client(&chain_a, "b.example.com");
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        assert!(conn.connect(name, tcp).await.is_err());
        // nome senza certificato: handshake rifiutato, il server continua a servire
        let (conn, name) = client(&chain_a, "sconosciuto.example.com");
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        assert!(conn.connect(name, tcp).await.is_err());
        let (conn, name) = client(&chain_a, "a.example.com");
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        assert!(conn.connect(name, tcp).await.is_ok());
    }
}
