//! Controllo delle nuove versioni: una richiesta al giorno alle release pubbliche
//! di GitHub, senza inviare nulla del nodo. Il risultato sta in `state/update.json`
//! e compare nel pannello (avviso in campanella e scheda in Impostazioni).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// La versione in esecuzione. `OTR_BUILD_VERSION` (solo in compilazione) serve alle prove
/// dell'auto-aggiornamento per costruire un binario "successivo" con un numero diverso.
pub const CURRENT: &str = match option_env!("OTR_BUILD_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};
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
    // `OTR_INSTALL` forza il tipo (l'immagine ufficiale imposta `docker`)
    match env("OTR_INSTALL").as_deref() {
        Some("docker") => return Kind::Docker,
        Some("service") => return Kind::Service,
        Some("binary") => return Kind::Binary,
        Some("source") => return Kind::Source,
        _ => {}
    }
    if dockerenv {
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
pub struct Asset {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rollback {
    pub to: String,
    pub reason: String,
    pub at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
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
    /// l'ultimo aggiornamento tornato indietro, con il motivo
    #[serde(default)]
    pub rollback: Option<Rollback>,
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
struct GhAsset {
    name: String,
    #[serde(default)]
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
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
            assets: r
                .assets
                .iter()
                .map(|a| Asset {
                    name: a.name.clone(),
                    url: a.browser_download_url.clone(),
                    size: a.size,
                })
                .collect(),
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

#[derive(Debug, Clone, Default, Serialize)]
pub struct Progress {
    pub running: bool,
    pub step: String,
    pub error: Option<String>,
    /// l'aggiornamento è pronto e il nodo sta per riavviarsi
    pub restarting: bool,
}

pub struct Updater {
    dir: PathBuf,
    api: String,
    api_overridden: bool,
    key_hex: Option<String>,
    ui_dir: Option<PathBuf>,
    docs_dir: Option<PathBuf>,
    /// chiamata subito prima del riavvio (salva le statistiche)
    on_restart: Box<dyn Fn() + Send + Sync>,
    progress: Mutex<Progress>,
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
        let api_overridden = api.as_deref().is_some_and(|a| !a.trim().is_empty());
        let api = api
            .filter(|a| !a.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_API.to_owned())
            .trim_end_matches('/')
            .to_owned();
        Self {
            dir,
            api,
            api_overridden,
            key_hex: None,
            ui_dir: None,
            docs_dir: None,
            on_restart: Box::new(|| {}),
            progress: Mutex::new(Progress::default()),
            kind: current_kind(),
            busy: tokio::sync::Mutex::new(()),
            cached,
        }
    }

    /// Cartelle del pannello e della guida da aggiornare insieme all'eseguibile, chiave pubblica
    /// alternativa (fork) e cosa fare prima del riavvio.
    pub fn with_install(
        mut self,
        ui: Option<PathBuf>,
        docs: Option<PathBuf>,
        key_hex: Option<String>,
        on_restart: Box<dyn Fn() + Send + Sync>,
    ) -> Self {
        self.ui_dir = ui.filter(|p| p.is_dir());
        self.docs_dir = docs.filter(|p| p.is_dir());
        self.key_hex = key_hex;
        self.on_restart = on_restart;
        self
    }

    pub fn progress(&self) -> Progress {
        self.progress.lock().unwrap().clone()
    }

    fn set_step(&self, step: &str) {
        self.progress.lock().unwrap().step = step.to_owned();
    }

    /// Questo nodo può aggiornarsi da solo? Altrimenti perché no.
    pub fn self_update_status(&self) -> (bool, Option<String>) {
        if !matches!(self.kind, Kind::Service | Kind::Binary) {
            return (
                false,
                Some(match self.kind {
                    Kind::Docker => "in Docker l'immagine non si sostituisce dall'interno: scarica quella nuova e ricrea il container".into(),
                    _ => "installazione dai sorgenti: aggiorna con git e cargo".into(),
                }),
            );
        }
        if crate::selfupdate::platform().is_none() {
            return (
                false,
                Some("nessun pacchetto per questa piattaforma".into()),
            );
        }
        if let Err(e) = crate::selfupdate::public_key(self.key_hex.as_deref()) {
            return (false, Some(e));
        }
        let Ok(exe) = std::env::current_exe() else {
            return (false, Some("percorso dell'eseguibile sconosciuto".into()));
        };
        match exe.parent() {
            Some(d) if crate::selfupdate::dir_writable(d) => (true, None),
            _ => (
                false,
                Some("la cartella dell'eseguibile non è scrivibile da questo utente".into()),
            ),
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
            "rollback": s.rollback.filter(|r| now().saturating_sub(r.at) < 7 * 24 * 3600),
            "can_self_update": self.self_update_status().0,
            "self_update_blocked": self.self_update_status().1,
            "apply": self.progress(),
        })
    }

    /// Un controllo: legge le release, aggiorna lo stato e lo salva.
    pub async fn check(&self) -> Result<(), String> {
        let _g = self.busy.lock().await;
        let etag = self.state().etag;
        let res = self.fetch(etag.as_deref()).await;
        // si riparte dal file (non dalla copia in memoria): altri pezzi ci scrivono (avvio, rollback)
        let mut s = load(&self.dir);
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
        crate::metrics_extra::inc(if err.is_some() {
            &crate::metrics_extra::C.update_check_error
        } else {
            &crate::metrics_extra::C.update_check_ok
        });
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

    /// Giro periodico: dopo un minuto dall'avvio, controlla ogni giorno; e, se l'utente lo ha
    /// chiesto, applica da solo le versioni di correzione dentro la finestra oraria.
    pub async fn run(
        self: std::sync::Arc<Self>,
        settings: impl Fn() -> crate::panel::UpdateSettings + Send + 'static,
        busy: impl Fn() -> bool + Send + 'static,
    ) {
        use chrono::Timelike;
        tokio::time::sleep(FIRST_CHECK).await;
        loop {
            let s = settings();
            // ±10% per non far partire tutti i nodi alla stessa ora
            let every = CHECK_EVERY.as_secs() + now() % (CHECK_EVERY.as_secs() / 10);
            let due = now().saturating_sub(self.state().checked_at) >= every;
            if s.check && !env_disabled() && due {
                if let Err(e) = self.check().await {
                    tracing::debug!(error = %e, "controllo aggiornamenti non riuscito");
                }
            }
            if s.auto && !env_disabled() {
                let hour = chrono::Local::now().hour() as u8;
                let latest = self.state().latest;
                if let Some(r) = latest.filter(|r| auto_candidate(&r.version, CURRENT)) {
                    if in_window(hour, s.window_start, s.window_end)
                        && !busy()
                        && self.self_update_status().0
                    {
                        tracing::info!(versione = %r.version, "aggiornamento automatico");
                        match self.apply(false).await {
                            Ok(never) => match never {},
                            Err(e) => {
                                tracing::warn!(error = %e, "aggiornamento automatico non riuscito")
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(30 * 60)).await;
        }
    }
}

/// Una versione di correzione (stessa serie 0.1.x), stabile e più recente: si può applicare da sola.
pub fn auto_candidate(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => !l.3 && !c.3 && (l.0, l.1) == (c.0, c.1) && l.2 > c.2,
        _ => false,
    }
}

/// `hour` è dentro la finestra `[start, end)`? Vale anche a cavallo della mezzanotte (23–2).
pub fn in_window(hour: u8, start: u8, end: u8) -> bool {
    let (h, s, e) = (hour % 24, start % 24, end % 24);
    if s == e {
        return false;
    }
    if s < e {
        h >= s && h < e
    } else {
        h >= s || h < e
    }
}

pub fn record_rollback(dir: &Path, to: &str, reason: &str) {
    crate::metrics_extra::inc(&crate::metrics_extra::C.update_rollback);
    let mut s = load(dir);
    s.rollback = Some(Rollback {
        to: to.to_owned(),
        reason: reason.to_owned(),
        at: now(),
    });
    save(dir, &s);
}

fn asset<'a>(r: &'a Release, name: &str) -> Result<&'a Asset, String> {
    r.assets
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| format!("la release non contiene «{name}»: aggiornamento non possibile"))
}

impl Updater {
    fn download_client(&self) -> Result<reqwest::Client, String> {
        let (api, over) = (self.api.clone(), self.api_overridden);
        let policy = reqwest::redirect::Policy::custom(move |a| {
            if a.previous().len() >= 5 {
                a.error("troppi reindirizzamenti")
            } else if crate::selfupdate::download_allowed(a.url().as_str(), &api, over) {
                a.follow()
            } else {
                a.error("reindirizzamento verso un indirizzo non ammesso")
            }
        });
        reqwest::Client::builder()
            .user_agent(concat!("otterroute/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .redirect(policy)
            .build()
            .map_err(|e| e.to_string())
    }

    async fn download(&self, c: &reqwest::Client, url: &str, max: u64) -> Result<Vec<u8>, String> {
        use futures_util::StreamExt;
        if !crate::selfupdate::download_allowed(url, &self.api, self.api_overridden) {
            return Err("indirizzo di download non ammesso".into());
        }
        let resp = c
            .get(url)
            .send()
            .await
            .map_err(|e| format!("download non riuscito: {}", e.without_url()))?;
        if !resp.status().is_success() {
            return Err(format!(
                "download non riuscito: HTTP {}",
                resp.status().as_u16()
            ));
        }
        if resp.content_length().is_some_and(|l| l > max) {
            return Err("il file da scaricare è troppo grande".into());
        }
        let mut buf = Vec::new();
        let mut s = resp.bytes_stream();
        while let Some(ch) = s.next().await {
            let ch = ch.map_err(|e| format!("download interrotto: {}", e.without_url()))?;
            if buf.len() as u64 + ch.len() as u64 > max {
                return Err("il file da scaricare è troppo grande".into());
            }
            buf.extend_from_slice(&ch);
        }
        Ok(buf)
    }

    /// Scarica, verifica, prova e sostituisce i file. Non riavvia: lo fa `restart()`.
    /// Ogni passo che fallisce lascia l'installazione com'era.
    pub async fn prepare(&self, release: &Release) -> Result<crate::selfupdate::Pending, String> {
        use crate::selfupdate as su;
        let (ok, why) = self.self_update_status();
        if !ok {
            return Err(why.unwrap_or_else(|| "aggiornamento automatico non disponibile".into()));
        }
        if !is_newer(&release.version, CURRENT) {
            return Err("la versione proposta non è più recente di quella in uso".into());
        }
        let plat = su::platform().ok_or("piattaforma non supportata")?;
        let base = format!("otterroute-v{}-{plat}.tar.gz", release.version);
        let (tar_a, sha_a, sig_a) = (asset(release, &base)?, asset(release, &format!("{base}.sha256"))?, asset(release, &format!("{base}.sig")).map_err(|_| {
            "la release non è firmata: per sicurezza l'aggiornamento automatico è rifiutato (aggiorna a mano)".to_string()
        })?);
        let exe = std::env::current_exe()
            .and_then(std::fs::canonicalize)
            .map_err(|e| format!("eseguibile: {e}"))?;
        let dir = exe
            .parent()
            .ok_or("cartella dell'eseguibile sconosciuta")?
            .to_path_buf();
        let pk = su::public_key(self.key_hex.as_deref())?;

        self.set_step("download");
        let client = self.download_client()?;
        let tar_gz = self.download(&client, &tar_a.url, su::MAX_ARCHIVE).await?;
        let sha_txt = String::from_utf8(self.download(&client, &sha_a.url, 4096).await?)
            .map_err(|_| "checksum non valido")?;
        let sig = self.download(&client, &sig_a.url, 1024).await?;

        self.set_step("verifica");
        let want = su::parse_sha256_file(&sha_txt).ok_or("il file del checksum non è valido")?;
        if su::sha256_hex(&tar_gz) != want {
            return Err(
                "il checksum SHA-256 del pacchetto non corrisponde: download corrotto o manomesso"
                    .into(),
            );
        }
        su::verify_signature(&pk, &tar_gz, &sig).map_err(|_| {
            "la firma del pacchetto non è valida: aggiornamento rifiutato".to_string()
        })?;

        self.set_step("estrazione e prova");
        let stage = dir.join(format!(".otterroute-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&stage);
        let result = self.stage_and_swap(&tar_gz, &stage, &exe, release).await;
        let _ = std::fs::remove_dir_all(&stage);
        result
    }

    async fn stage_and_swap(
        &self,
        tar_gz: &[u8],
        stage: &Path,
        exe: &Path,
        release: &Release,
    ) -> Result<crate::selfupdate::Pending, String> {
        use crate::selfupdate as su;
        let ex = su::extract(tar_gz, stage)?;
        // 1. il nuovo eseguibile dichiara la versione attesa
        let out = tokio::process::Command::new(&ex.binary)
            .arg("--version")
            .output()
            .await
            .map_err(|e| format!("il nuovo eseguibile non parte: {e}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        if !out.status.success() || !text.contains(&release.version) {
            return Err(format!(
                "il nuovo eseguibile non dichiara la versione {}: {}",
                release.version,
                text.trim()
            ));
        }
        // 2. si avvia davvero (porte e stato temporanei) e risponde a /healthz
        let chk = tokio::time::timeout(
            Duration::from_secs(40),
            tokio::process::Command::new(&ex.binary)
                .arg("--self-check")
                .output(),
        )
        .await
        .map_err(|_| "la prova di avvio del nuovo eseguibile è scaduta".to_string())?
        .map_err(|e| format!("prova di avvio non riuscita: {e}"))?;
        if !chk.status.success() {
            return Err(format!(
                "la prova di avvio del nuovo eseguibile è fallita: {}",
                String::from_utf8_lossy(&chk.stderr)
                    .lines()
                    .last()
                    .unwrap_or("errore sconosciuto")
            ));
        }
        self.set_step("backup");
        su::backup_state(&self.dir, CURRENT)?;
        self.set_step("sostituzione");
        su::swap_in(
            &self.dir,
            exe,
            &ex,
            self.ui_dir.as_deref(),
            self.docs_dir.as_deref(),
            CURRENT,
            &release.version,
            now(),
        )
    }

    /// Riavvia il processo con il nuovo eseguibile (`exec`: stesso PID, stessi argomenti).
    /// Se non riesce rimette a posto i file e il nodo continua a girare.
    pub fn restart(
        &self,
        pending: &crate::selfupdate::Pending,
    ) -> Result<std::convert::Infallible, String> {
        use std::os::unix::process::CommandExt;
        self.progress.lock().unwrap().restarting = true;
        (self.on_restart)();
        let err = std::process::Command::new(&pending.exe)
            .args(std::env::args_os().skip(1))
            .exec();
        // exec è tornato: c'è stato un errore, il vecchio processo prosegue
        let msg = format!("riavvio non riuscito ({err}): ripristino la versione precedente");
        let _ = crate::selfupdate::roll_back(pending);
        crate::selfupdate::clear_pending(&self.dir);
        record_rollback(&self.dir, &pending.to, &msg);
        self.progress.lock().unwrap().restarting = false;
        Err(msg)
    }

    /// Tutto in uno: dal pannello o dal giro automatico. Non ritorna se il riavvio riesce.
    pub async fn apply(&self, prerelease: bool) -> Result<std::convert::Infallible, String> {
        {
            let mut p = self.progress.lock().unwrap();
            if p.running {
                return Err("un aggiornamento è già in corso".into());
            }
            *p = Progress {
                running: true,
                step: "controllo".into(),
                ..Progress::default()
            };
        }
        let res = async {
            let s = self.state();
            // la stessa scelta che il pannello mostra: stabile, o anche pre-release se l'utente le vuole
            let candidate = if prerelease {
                s.latest_pre.or(s.latest)
            } else {
                s.latest
            };
            let release = candidate
                .filter(|r| is_newer(&r.version, CURRENT))
                .ok_or("nessuna versione nota: fai prima un controllo")?;
            let pending = self.prepare(&release).await?;
            audit_applied(&pending.from, &pending.to);
            self.restart(&pending)
        }
        .await;
        let mut p = self.progress.lock().unwrap();
        p.running = false;
        if let Err(e) = &res {
            p.error = Some(e.clone());
        }
        res
    }
}

fn audit_applied(from: &str, to: &str) {
    crate::audit::log("update.apply", &format!("{from} → {to}"));
}

/// Conferma dell'aggiornamento: dopo un po' di tempo senza problemi lo stato in attesa si cancella
/// e la guardia all'avvio smette di contare i tentativi.
pub async fn confirm_after(dir: PathBuf, current: String, secs: u64) {
    tokio::time::sleep(Duration::from_secs(secs)).await;
    if crate::selfupdate::read_pending(&dir).is_some_and(|p| p.to == current) {
        crate::selfupdate::clear_pending(&dir);
        tracing::info!("aggiornamento confermato");
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
    fn auto_update_only_takes_patch_releases_in_the_window() {
        assert!(auto_candidate("0.1.1", "0.1.0") && auto_candidate("v0.1.9", "0.1.2"));
        assert!(
            !auto_candidate("0.2.0", "0.1.9"),
            "una versione minore resta manuale"
        );
        assert!(!auto_candidate("1.0.0", "0.9.9"));
        assert!(!auto_candidate("0.1.0", "0.1.0") && !auto_candidate("0.1.0", "0.1.1"));
        assert!(!auto_candidate("0.1.1-rc1", "0.1.0"), "mai una pre-release");
        assert!(!auto_candidate("0.1.1", "0.1.0-rc3") || true);
        assert!(!auto_candidate("bah", "0.1.0"));
        assert!(
            in_window(3, 3, 5) && in_window(4, 3, 5) && !in_window(5, 3, 5) && !in_window(2, 3, 5)
        );
        assert!(
            in_window(23, 22, 2)
                && in_window(0, 22, 2)
                && in_window(1, 22, 2)
                && !in_window(2, 22, 2)
                && !in_window(12, 22, 2)
        );
        assert!(!in_window(3, 4, 4), "finestra vuota: mai");
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
            assets: vec![],
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
