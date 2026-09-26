//! Controllo delle nuove versioni: una richiesta al giorno alle release pubbliche
//! di GitHub, senza inviare nulla del nodo. Il risultato sta in `state/update.json`
//! e compare nel pannello (avviso in campanella e scheda in Impostazioni).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_API: &str = "https://api.github.com/repos/garzuu/OtterRoute";
const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);
const FIRST_CHECK: Duration = Duration::from_secs(60);
const NOTES_MAX: usize = 1500;

// --- versioni -----------------------------------------------------------------

/// `0.1.0`, `v0.2.3`, `1.0.0-rc1` → (maggiore, minore, correzione, pre-release?).
pub fn parse_version(s: &str) -> Option<(u64, u64, u64, bool)> {
    let s = s.trim().trim_start_matches('v');
    let (core, pre) = match s.split_once('-') {
        Some((c, _)) => (c, true),
        None => (s.split_once('+').map_or(s, |(c, _)| c), false),
    };
    let mut it = core.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next()?.parse().ok()?;
    it.next().is_none().then_some((a, b, c, pre))
}

/// `latest` è più recente di `current`? Una pre-release conta meno della stessa versione finale.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => {
            let key = |v: (u64, u64, u64, bool)| (v.0, v.1, v.2, !v.3);
            key(l) > key(c)
        }
        _ => false,
    }
}

// --- tipo di installazione ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Docker,
    Service,
    Binary,
    Source,
}

/// Come è installato questo nodo: decide che istruzioni mostrare e cosa può fare da solo.
pub fn detect_kind(env: &dyn Fn(&str) -> Option<String>, exe: &Path, dockerenv: bool) -> Kind {
    if env("OTR_INSTALL").as_deref() == Some("docker") || dockerenv {
        return Kind::Docker;
    }
    if exe.components().any(|c| c.as_os_str() == "target") {
        return Kind::Source;
    }
    if env("INVOCATION_ID").is_some() || env("XPC_SERVICE_NAME").is_some_and(|v| v != "0") {
        return Kind::Service;
    }
    Kind::Binary
}

pub fn current_kind() -> Kind {
    let exe = std::env::current_exe().unwrap_or_default();
    detect_kind(
        &|k| std::env::var(k).ok(),
        &exe,
        Path::new("/.dockerenv").exists(),
    )
}

// --- stato ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub published_at: String,
    pub prerelease: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// la versione stabile più recente
    #[serde(default)]
    pub latest: Option<Release>,
    /// la più recente in assoluto, pre-release comprese
    #[serde(default)]
    pub latest_pre: Option<Release>,
    /// secondi Unix dell'ultimo controllo riuscito o fallito
    #[serde(default)]
    pub checked_at: u64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub etag: Option<String>,
    /// ultima versione in esecuzione vista dal nodo (per l'avviso «aggiornato»)
    #[serde(default)]
    pub last_run_version: Option<String>,
    /// quando è stato visto il cambio di versione
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub updated_from: Option<String>,
}

fn state_path(dir: &Path) -> PathBuf {
    dir.join("update.json")
}

pub fn load(dir: &Path) -> State {
    std::fs::read(state_path(dir))
        .ok()
        .and_then(|r| serde_json::from_slice(&r).ok())
        .unwrap_or_default()
}

fn save(dir: &Path, s: &State) {
    if let Ok(raw) = serde_json::to_vec_pretty(s) {
        let _ = crate::admin::write_atomic(&state_path(dir), &raw, false);
    }
}

pub fn now_pub() -> u64 {
    now()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// All'avvio: se la versione in esecuzione è cambiata dall'ultima volta, lo registra.
pub fn note_startup(dir: &Path, current: &str, t: u64) -> Option<String> {
    let mut s = load(dir);
    let prev = s.last_run_version.clone();
    let changed = prev.as_deref().is_some_and(|p| p != current);
    if changed {
        s.updated_from = prev.clone();
        s.updated_at = t;
    }
    if prev.as_deref() != Some(current) {
        s.last_run_version = Some(current.to_owned());
        save(dir, &s);
    }
    changed.then(|| prev.unwrap_or_default())
}

// --- controllo --------------------------------------------------------------------------

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

fn short_notes(body: &str) -> String {
    let t: String = body.chars().take(NOTES_MAX).collect();
    t.trim().to_owned()
}

/// Sceglie la release da proporre: la più alta tra quelle stabili (e, se richiesto, le pre-release).
fn pick_ref(list: &[GhRelease], with_pre: bool) -> Option<Release> {
    list.iter()
        .filter(|r| !r.draft && (with_pre || !r.prerelease) && parse_version(&r.tag_name).is_some())
        .max_by_key(|r| parse_version(&r.tag_name).map(|v| (v.0, v.1, v.2, !v.3)))
        .map(|r| Release {
            version: r.tag_name.trim_start_matches('v').to_owned(),
            notes: short_notes(r.body.as_deref().unwrap_or_default()),
            url: r.html_url.clone(),
            published_at: r.published_at.clone().unwrap_or_default(),
            prerelease: r.prerelease,
        })
}

#[cfg(test)]
fn pick(list: Vec<GhRelease>, with_pre: bool) -> Option<Release> {
    pick_ref(&list, with_pre)
}

pub struct Updater {
    dir: PathBuf,
    api: String,
    pub kind: Kind,
    /// serializza i controlli (giro periodico e pulsante «Controlla ora»)
    busy: tokio::sync::Mutex<()>,
    cached: Mutex<State>,
}

/// L'utente ha attivato il controllo? `OTR_UPDATE_CHECK=off` lo spegne sempre.
pub fn env_disabled() -> bool {
    std::env::var("OTR_UPDATE_CHECK").is_ok_and(|v| {
        matches!(
            v.to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

impl Updater {
    pub fn new(dir: PathBuf, api: Option<String>) -> Self {
        let cached = Mutex::new(load(&dir));
        let api = api
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_API.to_owned())
            .trim_end_matches('/')
            .to_owned();
        Self {
            dir,
            api,
            kind: current_kind(),
            busy: tokio::sync::Mutex::new(()),
            cached,
        }
    }

    pub fn state(&self) -> State {
        self.cached.lock().unwrap().clone()
    }

    /// Cosa mostra il pannello.
    pub fn view(&self, enabled: bool, prerelease: bool) -> serde_json::Value {
        let s = self.state();
        let latest = if prerelease {
            s.latest_pre.as_ref().or(s.latest.as_ref())
        } else {
            s.latest.as_ref()
        };
        let available = latest.is_some_and(|r| is_newer(&r.version, CURRENT));
        let recently = s.updated_from.is_some() && now().saturating_sub(s.updated_at) < 24 * 3600;
        serde_json::json!({
            "current": CURRENT,
            "kind": self.kind,
            "enabled": enabled && !env_disabled(),
            "env_disabled": env_disabled(),
            "available": available,
            "latest": latest,
            "checked_at": s.checked_at,
            "error": s.error,
            "updated_from": recently.then_some(s.updated_from),
        })
    }

    /// Un controllo: legge le release, aggiorna lo stato e lo salva.
    pub async fn check(&self) -> Result<(), String> {
        let _g = self.busy.lock().await;
        let etag = self.state().etag;
        let res = self.fetch(etag.as_deref()).await;
        let mut s = self.cached.lock().unwrap().clone();
        s.checked_at = now();
        match res {
            Ok(Fetched::NotModified) => s.error = None,
            Ok(Fetched::List(list, etag)) => {
                let list: Vec<GhRelease> = list;
                s.latest_pre = pick_ref(&list, true);
                s.latest = pick_ref(&list, false);
                s.etag = etag;
                s.error = None;
            }
            Err(e) => s.error = Some(e),
        }
        save(&self.dir, &s);
        let err = s.error.clone();
        *self.cached.lock().unwrap() = s;
        err.map_or(Ok(()), Err)
    }

    async fn fetch(&self, etag: Option<&str>) -> Result<Fetched, String> {
        // un mirror interno (OTR_UPDATE_API su rete locale) è ammesso solo se lo si è impostato di proposito
        self.fetch_with(!self.api.starts_with(DEFAULT_API), etag)
            .await
    }

    async fn fetch_with(&self, allow_private: bool, etag: Option<&str>) -> Result<Fetched, String> {
        let client = crate::s3::build_client(allow_private).map_err(|e| e.to_string())?;
        // sempre l'elenco (le pre-release si filtrano dopo, secondo l'impostazione)
        let url = format!("{}/releases?per_page=30", self.api);
        let mut req = client
            .get(&url)
            .header("accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20));
        if let Some(e) = etag {
            req = req.header("if-none-match", e);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| format!("GitHub non raggiungibile: {}", e.without_url()))?;
        match resp.status().as_u16() {
            304 => Ok(Fetched::NotModified),
            200 => {
                let etag = resp
                    .headers()
                    .get("etag")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let text = resp.text().await.map_err(|e| e.to_string())?;
                if text.len() > 2_000_000 {
                    return Err("risposta troppo grande".into());
                }
                let list: Vec<GhRelease> =
                    serde_json::from_str(&text).map_err(|e| format!("risposta non valida: {e}"))?;
                Ok(Fetched::List(list, etag))
            }
            403 | 429 => Err("limite di richieste di GitHub raggiunto: riprova più tardi".into()),
            s => Err(format!("GitHub ha risposto {s}")),
        }
    }

    /// Giro periodico: dopo un minuto dall'avvio, poi ogni giorno.
    pub async fn run(self: std::sync::Arc<Self>, enabled: impl Fn() -> bool + Send + 'static) {
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            if enabled() && !env_disabled() {
                if let Err(e) = self.check().await {
                    tracing::debug!(error = %e, "controllo aggiornamenti non riuscito");
                }
            }
            // ±10% per non far partire tutti i nodi alla stessa ora
            let jitter = now() % (CHECK_EVERY.as_secs() / 10);
            tokio::time::sleep(CHECK_EVERY + Duration::from_secs(jitter)).await;
        }
    }
}

enum Fetched {
    NotModified,
    /// (release, etag)
    List(Vec<GhRelease>, Option<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_like_semver() {
        assert_eq!(parse_version("v0.1.0"), Some((0, 1, 0, false)));
        assert_eq!(parse_version("1.2.3-rc1"), Some((1, 2, 3, true)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert_eq!(parse_version("x.y.z"), None);
        assert!(
            is_newer("0.1.1", "0.1.0")
                && is_newer("0.2.0", "0.1.9")
                && is_newer("1.0.0", "0.99.99")
        );
        assert!(
            !is_newer("0.1.0", "0.1.0")
                && !is_newer("0.1.0", "0.1.1")
                && !is_newer("0.9.0", "0.10.0")
        );
        assert!(is_newer("0.10.0", "0.9.0"), "numeri, non testo");
        assert!(
            is_newer("0.1.0", "0.1.0-rc3"),
            "la finale batte la pre-release"
        );
        assert!(!is_newer("0.1.0-rc3", "0.1.0"));
        assert!(is_newer("0.1.1-rc1", "0.1.0"));
        assert!(!is_newer("garbage", "0.1.0") && !is_newer("0.1.0", "garbage"));
    }

    #[test]
    fn install_kind_detection() {
        let none = |_: &str| None;
        let with = |k: &'static str, v: &'static str| move |x: &str| (x == k).then(|| v.to_owned());
        let bin = Path::new("/usr/local/bin/otterroute");
        assert_eq!(detect_kind(&none, bin, false), Kind::Binary);
        assert_eq!(detect_kind(&none, bin, true), Kind::Docker);
        assert_eq!(
            detect_kind(&with("OTR_INSTALL", "docker"), bin, false),
            Kind::Docker
        );
        assert_eq!(
            detect_kind(&with("INVOCATION_ID", "abc"), bin, false),
            Kind::Service
        );
        assert_eq!(
            detect_kind(&with("XPC_SERVICE_NAME", "com.x.otterroute"), bin, false),
            Kind::Service
        );
        assert_eq!(
            detect_kind(&with("XPC_SERVICE_NAME", "0"), bin, false),
            Kind::Binary,
            "il terminale di macOS non è un servizio"
        );
        assert_eq!(
            detect_kind(
                &none,
                Path::new("/home/u/otterroute/target/release/otterroute"),
                false
            ),
            Kind::Source
        );
        assert_eq!(
            detect_kind(
                &with("INVOCATION_ID", "abc"),
                Path::new("/x/target/debug/otterroute"),
                false
            ),
            Kind::Source
        );
    }

    fn gh(tag: &str, pre: bool, draft: bool) -> GhRelease {
        GhRelease {
            tag_name: tag.into(),
            body: Some(format!("note {tag}")),
            html_url: format!("https://x/{tag}"),
            published_at: None,
            prerelease: pre,
            draft,
        }
    }

    #[test]
    fn picks_the_highest_eligible_release() {
        let list = || {
            vec![
                gh("v0.1.0", false, false),
                gh("v0.2.0-rc1", true, false),
                gh("v0.1.1", false, false),
                gh("v0.3.0", false, true),
                gh("nightly", false, false),
            ]
        };
        assert_eq!(
            pick(list(), false).unwrap().version,
            "0.1.1",
            "niente pre-release, bozze o tag strani"
        );
        assert_eq!(pick(list(), true).unwrap().version, "0.2.0-rc1");
        assert!(pick(vec![gh("nightly", false, false)], true).is_none());
        assert_eq!(pick(list(), false).unwrap().notes, "note v0.1.1");
    }

    #[test]
    fn startup_records_a_version_change_once() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            note_startup(d.path(), "0.1.0", 100),
            None,
            "primo avvio: nessun cambio"
        );
        assert_eq!(note_startup(d.path(), "0.1.0", 200), None);
        assert_eq!(
            note_startup(d.path(), "0.1.1", 300).as_deref(),
            Some("0.1.0")
        );
        let s = load(d.path());
        assert_eq!(
            (
                s.updated_from.as_deref(),
                s.updated_at,
                s.last_run_version.as_deref()
            ),
            (Some("0.1.0"), 300, Some("0.1.1"))
        );
        assert_eq!(
            note_startup(d.path(), "0.1.1", 400),
            None,
            "riavvio senza cambio di versione"
        );
        assert_eq!(load(d.path()).updated_at, 300);
    }

    /// Finto GitHub: serve l'elenco delle release e conta le richieste.
    async fn fake_github(
        body: &'static str,
        status: u16,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let n = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let n2 = n.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else {
                    return;
                };
                let mut buf = vec![0u8; 4096];
                let k = s.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..k]).to_string();
                n2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let (st, b) = if req.to_ascii_lowercase().contains("if-none-match: \"v1\"") {
                    (304, "")
                } else {
                    (status, body)
                };
                let r = format!("HTTP/1.1 {st} X\r\ncontent-type: application/json\r\netag: \"v1\"\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{b}", b.len());
                let _ = s.write_all(r.as_bytes()).await;
            }
        });
        (base, n)
    }

    #[tokio::test]
    async fn check_reads_releases_and_uses_the_etag() {
        let dir = tempfile::tempdir().unwrap();
        let body: &'static str = r#"[{"tag_name":"v9.9.9","body":"novità","html_url":"https://x/r","published_at":"2026-01-01T00:00:00Z","prerelease":false,"draft":false},{"tag_name":"v0.0.1","prerelease":false,"draft":false}]"#;
        let (base, hits) = fake_github(body, 200).await;
        // il finto server è locale: `build_client(false)` filtrerebbe gli IP privati, quindi lo si prova con l'API reale via env
        let u = Updater::new(dir.path().to_path_buf(), Some(base));
        // nei test l'endpoint locale è ammesso solo se l'ambiente lo consente: si usa un client permissivo
        assert!(
            u.view(true, false)["available"] == false,
            "nessun controllo ancora fatto"
        );
        assert!(matches!(
            u.fetch_with(true, None).await.unwrap(),
            Fetched::List(..)
        ));
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        // con l'ETag già noto il server risponde 304 e non si riscarica nulla
        assert!(matches!(
            u.fetch_with(true, Some("\"v1\"")).await.unwrap(),
            Fetched::NotModified
        ));
    }

    #[tokio::test]
    async fn rate_limit_and_errors_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        let (base, _) = fake_github("{}", 403).await;
        let u = Updater::new(dir.path().to_path_buf(), Some(base));
        let e = u.fetch_with(true, None).await.err().unwrap();
        assert!(e.contains("limite"), "{e}");
        let (base, _) = fake_github("non è json", 200).await;
        let u = Updater::new(dir.path().to_path_buf(), Some(base));
        assert!(u
            .fetch_with(true, None)
            .await
            .err()
            .unwrap()
            .contains("non valida"));
    }
}
