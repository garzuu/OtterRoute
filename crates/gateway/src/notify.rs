//! Notifiche di avvisi ed errori (email SMTP e Telegram).
//!
//! Gli avvisi sono quelli della campanella del pannello (domini e bucket): il
//! motore li ricalcola ogni 30 secondi dallo stato del pannello, li confronta con
//! quelli già visti e notifica solo i **cambi di stato**: un problema che resta
//! aperto oltre l'attesa configurata, il suo peggioramento e il ripristino.

mod email;
mod telegram;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::dns::Stage;
use crate::panel::{self, Panel};

pub const TICK: Duration = Duration::from_secs(30);
const LOG_KEEP: usize = 50;
const TEST_INTERVAL: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Problemi (gli stessi avvisi di web/src/alerts.ts)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Warn,
    Error,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Level::Warn => "avviso",
            Level::Error => "errore",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// stesso identificativo dell'avviso nel pannello: `d:<host>` o `b:<id>`
    pub id: String,
    pub level: Level,
    pub title: String,
    pub text: String,
}

fn domain_state(d: &panel::Domain) -> &'static str {
    if d.verified {
        return "verified";
    }
    if !d.ever_verified {
        return "pending";
    }
    let fails = |s: &Stage| s.status == "fail";
    let reach_only = d.stages.iter().any(|s| s.id == "reach" && fails(s))
        && d.stages.iter().all(|s| s.id == "reach" || !fails(s));
    if reach_only {
        "unreachable"
    } else {
        "dns_error"
    }
}

/// Tutto ciò che oggi richiede attenzione. Regole e testi allineati alla campanella.
pub fn problems(p: &Panel, certs: &[crate::tls::CertInfo], now: u64) -> Vec<Problem> {
    let mut out = Vec::new();
    for d in &p.domains {
        let st = domain_state(d);
        if st == "verified" {
            continue;
        }
        let failing_reach = d
            .stages
            .iter()
            .find(|s| s.status == "fail")
            .map(|s| s.id.as_str())
            == Some("reach");
        let title = if failing_reach {
            "Il nodo non risponde attraverso il dominio"
        } else {
            "Il dominio non risolve"
        };
        out.push(Problem {
            id: format!("d:{}", d.host),
            level: if st == "pending" {
                Level::Warn
            } else {
                Level::Error
            },
            title: format!("{title} · {}", d.host),
            text: d.message.clone(),
        });
    }
    // certificati automatici: scaduti, in scadenza o non emessi
    if p.settings.acme.enabled {
        for d in p
            .domains
            .iter()
            .filter(|d| d.verified && crate::acme::certifiable(&d.host))
        {
            let Some(info) = certs.iter().find(|c| c.host == d.host) else {
                continue;
            };
            let (level, title, text) = match crate::tls::status(info, now) {
                "expired" => (
                    Level::Error,
                    "Certificato scaduto",
                    "Il certificato HTTPS è scaduto e il rinnovo non è riuscito.".to_owned(),
                ),
                "expiring" => (
                    Level::Warn,
                    "Certificato in scadenza",
                    "Scade tra meno di 14 giorni: il rinnovo automatico non è ancora riuscito."
                        .to_owned(),
                ),
                "error" => (
                    Level::Warn,
                    "Certificato non emesso",
                    info.error
                        .as_ref()
                        .map(|e| e.message.clone())
                        .unwrap_or_default(),
                ),
                _ => continue,
            };
            out.push(Problem {
                id: format!("c:{}", d.host),
                level,
                title: format!("{title} · {}", d.host),
                text,
            });
        }
    }
    for b in &p.buckets {
        let mk = |level, title: &str, text: String| Problem {
            id: format!("b:{}", b.id),
            level,
            title: format!("{title} · {}", b.name),
            text,
        };
        match &b.check {
            None => out.push(mk(
                Level::Warn,
                "Bucket non verificato",
                "Non è ancora stata provata la lettura.".into(),
            )),
            Some(c) if c.outcome == "unreachable" => out.push(mk(
                Level::Error,
                "Bucket non raggiungibile",
                c.message.clone(),
            )),
            Some(c) if c.outcome == "auth" => {
                out.push(mk(Level::Error, "Credenziali rifiutate", c.message.clone()))
            }
            Some(c) if !c.ok => out.push(mk(Level::Error, "Errore sul bucket", c.message.clone())),
            Some(c) if c.outcome != "found" => out.push(mk(
                Level::Warn,
                "File di prova non trovato",
                c.message.clone(),
            )),
            Some(_) => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Configurazione
// ---------------------------------------------------------------------------

/// Soglia di un canale: `errors` = solo errori; `warnings` = avvisi ed errori.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Threshold {
    Warnings,
    Errors,
}

impl Threshold {
    pub fn accepts(self, l: Level) -> bool {
        self == Threshold::Warnings || l == Level::Error
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Security {
    /// TLS implicito (di solito porta 465)
    Tls,
    /// STARTTLS (di solito porta 587)
    Starttls,
    /// nessuna cifratura: solo per relay locali
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmailCfg {
    pub enabled: bool,
    pub threshold: Threshold,
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub user: String,
    pub from: String,
    pub to: Vec<String>,
}

impl Default for EmailCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: Threshold::Errors,
            host: String::new(),
            port: 587,
            security: Security::Starttls,
            user: String::new(),
            from: String::new(),
            to: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelegramCfg {
    pub enabled: bool,
    pub threshold: Threshold,
    pub chat_ids: Vec<String>,
}

impl Default for TelegramCfg {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: Threshold::Errors,
            chat_ids: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// minuti di attesa prima di notificare un problema (evita i falsi allarmi)
    pub delay_min: u64,
    /// avvisa anche quando il problema rientra
    pub recovery: bool,
    /// ripete la notifica ogni N ore finché il problema resta aperto (0 = mai)
    pub reminder_hours: u64,
    pub email: EmailCfg,
    pub telegram: TelegramCfg,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            delay_min: 10,
            recovery: true,
            reminder_hours: 0,
            email: EmailCfg::default(),
            telegram: TelegramCfg::default(),
        }
    }
}

fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 253
        && h.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == ':' || c == '[' || c == ']'
        })
}

fn valid_chat_id(c: &str) -> bool {
    let num = c.strip_prefix('-').unwrap_or(c);
    (!num.is_empty() && num.len() <= 20 && num.chars().all(|x| x.is_ascii_digit()))
        || (c.starts_with('@')
            && c.len() >= 6
            && c[1..]
                .chars()
                .all(|x| x.is_ascii_alphanumeric() || x == '_'))
}

impl Config {
    /// Controlla i valori; l'errore è pronto da mostrare all'utente.
    pub fn validate(&self) -> Result<(), String> {
        if self.delay_min > 24 * 60 {
            return Err("l'attesa non può superare 24 ore".into());
        }
        if self.reminder_hours > 24 * 7 {
            return Err("il promemoria non può superare 7 giorni".into());
        }
        let e = &self.email;
        if e.enabled || !e.host.is_empty() {
            if !valid_host(&e.host) {
                return Err("server SMTP non valido: indica solo il nome (es. smtp.example.com), senza schema".into());
            }
            if e.port == 0 {
                return Err("porta SMTP non valida".into());
            }
        }
        if e.enabled {
            email::parse_mailbox(&e.from)
                .map_err(|_| "indirizzo del mittente non valido".to_string())?;
            if e.to.is_empty() {
                return Err("indica almeno un destinatario email".into());
            }
            if e.to.len() > 10 {
                return Err("al massimo 10 destinatari email".into());
            }
        }
        for a in &e.to {
            email::parse_mailbox(a).map_err(|_| format!("indirizzo email non valido: {a}"))?;
        }
        if e.user.chars().any(|c| c.is_control()) {
            return Err("utente SMTP non valido".into());
        }
        let t = &self.telegram;
        if t.enabled && t.chat_ids.is_empty() {
            return Err("indica almeno una chat Telegram".into());
        }
        if t.chat_ids.len() > 10 {
            return Err("al massimo 10 chat Telegram".into());
        }
        for c in &t.chat_ids {
            if !valid_chat_id(c) {
                return Err(format!(
                    "chat Telegram non valida: {c} (numero, o @nomecanale)"
                ));
            }
        }
        Ok(())
    }
}

/// Segreti dei canali, in file 0600 come le chiavi dei bucket.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Secrets {
    #[serde(default)]
    pub smtp_password: String,
    #[serde(default)]
    pub telegram_token: String,
}

fn config_path(d: &Path) -> PathBuf {
    d.join("notify.json")
}
fn secrets_path(d: &Path) -> PathBuf {
    d.join("secrets").join("_notify.json")
}
fn state_path(d: &Path) -> PathBuf {
    d.join("notify-state.json")
}
fn log_path(d: &Path) -> PathBuf {
    d.join("notify-log.json")
}

fn read_json<T: for<'a> Deserialize<'a> + Default>(p: &Path) -> Result<T, String> {
    match std::fs::read(p) {
        Ok(raw) => serde_json::from_slice(&raw).map_err(|e| format!("{}: {e}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

pub fn load_config(state_dir: &Path) -> Result<Config, String> {
    read_json(&config_path(state_dir))
}

pub fn save_config(state_dir: &Path, c: &Config) -> std::io::Result<()> {
    let data = serde_json::to_vec_pretty(c).map_err(std::io::Error::other)?;
    crate::admin::write_atomic(&config_path(state_dir), &data, false)
}

pub fn load_secrets(state_dir: &Path) -> Secrets {
    read_json(&secrets_path(state_dir)).unwrap_or_default()
}

pub fn save_secrets(state_dir: &Path, s: &Secrets) -> std::io::Result<()> {
    let p = secrets_path(state_dir);
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let data = serde_json::to_vec_pretty(s).map_err(std::io::Error::other)?;
    crate::admin::write_atomic(&p, &data, true)
}

// ---------------------------------------------------------------------------
// Motore: cosa notificare
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Problem,
    Recovered,
    Reminder,
    Test,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub kind: Kind,
    pub level: Level,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tracked {
    level: Level,
    /// da quando il problema ha questo livello (secondi unix)
    since: u64,
    /// livello più alto già notificato
    notified: Option<Level>,
    last_notice: u64,
    title: String,
    text: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    tracked: HashMap<String, Tracked>,
}

/// Confronta i problemi attuali con quelli già visti. Pura: niente rete né orologio.
pub fn plan(state: &mut State, now: &[Problem], cfg: &Config, t: u64) -> Vec<Event> {
    let mut events = Vec::new();
    let delay = cfg.delay_min * 60;
    let mut seen: Vec<&str> = Vec::new();
    for p in now {
        seen.push(&p.id);
        let tr = state
            .tracked
            .entry(p.id.clone())
            .or_insert_with(|| Tracked {
                level: p.level,
                since: t,
                notified: None,
                last_notice: 0,
                title: p.title.clone(),
                text: p.text.clone(),
            });
        if p.level > tr.level {
            // peggiora: riparte l'attesa
            tr.level = p.level;
            tr.since = t;
        } else if p.level < tr.level {
            tr.level = p.level;
        }
        tr.title = p.title.clone();
        tr.text = p.text.clone();
        let waited = t.saturating_sub(tr.since) >= delay;
        if waited && tr.notified.is_none_or(|n| n < tr.level) {
            tr.notified = Some(tr.level);
            tr.last_notice = t;
            events.push(Event {
                kind: Kind::Problem,
                level: tr.level,
                title: tr.title.clone(),
                text: tr.text.clone(),
            });
        } else if cfg.reminder_hours > 0
            && tr.notified.is_some()
            && t.saturating_sub(tr.last_notice) >= cfg.reminder_hours * 3600
        {
            tr.last_notice = t;
            events.push(Event {
                kind: Kind::Reminder,
                level: tr.level,
                title: tr.title.clone(),
                text: tr.text.clone(),
            });
        }
    }
    let gone: Vec<String> = state
        .tracked
        .keys()
        .filter(|k| !seen.contains(&k.as_str()))
        .cloned()
        .collect();
    for id in gone {
        if let Some(tr) = state.tracked.remove(&id) {
            if let (Some(level), true) = (tr.notified, cfg.recovery) {
                events.push(Event {
                    kind: Kind::Recovered,
                    level,
                    title: tr.title,
                    text: "Il problema è rientrato.".into(),
                });
            }
        }
    }
    events
}

/// Oggetto e corpo del messaggio (uguali per email e Telegram).
pub fn format_message(e: &Event, node: &str, panel_url: Option<&str>) -> (String, String) {
    let tag = match e.kind {
        Kind::Problem => format!("OtterRoute · {}", e.level.label()),
        Kind::Reminder => format!("OtterRoute · {} ancora aperto", e.level.label()),
        Kind::Recovered => "OtterRoute · ripristinato".to_string(),
        Kind::Test => "OtterRoute · prova".to_string(),
    };
    let subject = format!("[{tag}] {}", e.title);
    let mut body = format!("{}\n\n{}\n\nNodo: {node}", e.title, e.text);
    if let Some(u) = panel_url {
        body.push_str(&format!("\nPannello: {u}"));
    }
    (subject, body)
}

// ---------------------------------------------------------------------------
// Registro degli invii
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub at: String,
    pub channel: String,
    pub kind: Kind,
    pub title: String,
    pub ok: bool,
    pub error: Option<String>,
}

pub struct Notifier {
    pub state_dir: PathBuf,
    pub node_id: String,
    /// indirizzo pubblico del pannello, se noto (per il link nei messaggi)
    pub panel_url: Option<String>,
    log: Mutex<Vec<LogEntry>>,
    last_test: Mutex<Option<Instant>>,
    telegram_base: String,
}

impl Notifier {
    pub fn new(state_dir: PathBuf, node_id: String, panel_url: Option<String>) -> Self {
        let log = read_json::<Vec<LogEntry>>(&log_path(&state_dir)).unwrap_or_default();
        Self {
            state_dir,
            node_id,
            panel_url,
            log: Mutex::new(log),
            last_test: Mutex::new(None),
            telegram_base: telegram::API_BASE.to_string(),
        }
    }

    pub fn log(&self) -> Vec<LogEntry> {
        self.log.lock().unwrap().iter().rev().cloned().collect()
    }

    /// Canali con notifiche attive il cui ultimo invio è fallito.
    pub fn failing_channels(&self) -> Vec<String> {
        let cfg = load_config(&self.state_dir).unwrap_or_default();
        let log = self.log.lock().unwrap();
        ["email", "telegram"]
            .into_iter()
            .filter(|ch| match *ch {
                "email" => cfg.email.enabled,
                _ => cfg.telegram.enabled,
            })
            .filter(|ch| {
                log.iter()
                    .rev()
                    .find(|l| l.channel == *ch)
                    .is_some_and(|l| !l.ok)
            })
            .map(str::to_string)
            .collect()
    }

    fn record(&self, e: LogEntry) {
        let mut log = self.log.lock().unwrap();
        log.push(e);
        let extra = log.len().saturating_sub(LOG_KEEP);
        log.drain(..extra);
        if let Ok(data) = serde_json::to_vec_pretty(&*log) {
            let _ = crate::admin::write_atomic(&log_path(&self.state_dir), &data, false);
        }
    }

    fn node_label(&self) -> String {
        format!("nodo {}", self.node_id.chars().take(8).collect::<String>())
    }

    /// Invia un evento a tutti i canali che ne accettano il livello.
    async fn dispatch(&self, cfg: &Config, secrets: &Secrets, ev: &Event) {
        let (subject, body) = format_message(ev, &self.node_label(), self.panel_url.as_deref());
        let mut jobs: Vec<&'static str> = Vec::new();
        if cfg.email.enabled && cfg.email.threshold.accepts(ev.level) {
            jobs.push("email");
        }
        if cfg.telegram.enabled && cfg.telegram.threshold.accepts(ev.level) {
            jobs.push("telegram");
        }
        for ch in jobs {
            let res = self.send_one(ch, cfg, secrets, &subject, &body).await;
            self.record(LogEntry {
                at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                channel: ch.into(),
                kind: ev.kind,
                title: ev.title.clone(),
                ok: res.is_ok(),
                error: res.err(),
            });
        }
    }

    async fn send_one(
        &self,
        channel: &str,
        cfg: &Config,
        s: &Secrets,
        subject: &str,
        body: &str,
    ) -> Result<(), String> {
        let mut last = String::new();
        for attempt in 0..3u64 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(2 * attempt)).await;
            }
            let r = match channel {
                "email" => email::send(&cfg.email, &s.smtp_password, subject, body).await,
                _ => {
                    telegram::send(
                        &self.telegram_base,
                        &cfg.telegram,
                        &s.telegram_token,
                        &format!("{subject}\n\n{body}"),
                    )
                    .await
                }
            };
            match r {
                Ok(()) => return Ok(()),
                Err(e) => last = e,
            }
        }
        tracing::warn!(canale = channel, errore = %last, "notifica non recapitata");
        Err(last)
    }

    /// Messaggio di prova con l'esito reale dell'invio (un solo tentativo, senza attese).
    pub async fn test(&self, channel: &str) -> Result<(), String> {
        {
            let mut last = self.last_test.lock().unwrap();
            if last.is_some_and(|t| t.elapsed() < TEST_INTERVAL) {
                return Err("attendi qualche secondo tra una prova e l'altra".into());
            }
            *last = Some(Instant::now());
        }
        let cfg = load_config(&self.state_dir)?;
        let secrets = load_secrets(&self.state_dir);
        let ev = Event {
            kind: Kind::Test,
            level: Level::Warn,
            title: "Messaggio di prova".into(),
            text: "Se leggi questo messaggio le notifiche funzionano.".into(),
        };
        let (subject, body) = format_message(&ev, &self.node_label(), self.panel_url.as_deref());
        let res = match channel {
            "email" if !cfg.email.host.is_empty() => {
                email::send(&cfg.email, &secrets.smtp_password, &subject, &body).await
            }
            "telegram" if !cfg.telegram.chat_ids.is_empty() => {
                telegram::send(
                    &self.telegram_base,
                    &cfg.telegram,
                    &secrets.telegram_token,
                    &format!("{subject}\n\n{body}"),
                )
                .await
            }
            "email" | "telegram" => Err("salva prima le impostazioni del canale".into()),
            _ => Err("canale sconosciuto".into()),
        };
        self.record(LogEntry {
            at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            channel: channel.into(),
            kind: Kind::Test,
            title: ev.title,
            ok: res.is_ok(),
            error: res.clone().err(),
        });
        res
    }

    /// Un giro del motore.
    pub async fn tick(&self, t: u64) {
        let Ok(panel) = panel::load(&self.state_dir) else {
            return;
        };
        let Ok(cfg) = load_config(&self.state_dir) else {
            return;
        };
        let mut state: State = read_json(&state_path(&self.state_dir)).unwrap_or_default();
        let certs: Vec<crate::tls::CertInfo> = panel
            .domains
            .iter()
            .map(|d| crate::tls::read_info(&self.state_dir, &d.host))
            .collect();
        let events = plan(&mut state, &problems(&panel, &certs, t), &cfg, t);
        if let Ok(data) = serde_json::to_vec_pretty(&state) {
            let _ = crate::admin::write_atomic(&state_path(&self.state_dir), &data, false);
        }
        if events.is_empty() || !(cfg.email.enabled || cfg.telegram.enabled) {
            return;
        }
        let secrets = load_secrets(&self.state_dir);
        for ev in &events {
            self.dispatch(&cfg, &secrets, ev).await;
        }
    }
}

pub async fn run(n: std::sync::Arc<Notifier>) {
    let mut tick = tokio::time::interval(TICK);
    loop {
        tick.tick().await;
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        n.tick(t).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::{Bucket, BucketCheck, Domain};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    fn domain(host: &str, verified: bool, ever: bool, fail: &[&str]) -> Domain {
        Domain {
            host: host.into(),
            verified,
            checked_at: None,
            records: vec![],
            message: "msg".into(),
            stages: fail
                .iter()
                .map(|id| Stage {
                    id: (*id).into(),
                    status: "fail".into(),
                    message: String::new(),
                    label: String::new(),
                })
                .collect(),
            ever_verified: ever,
            since: None,
            redirect_https: false,
        }
    }

    fn bucket(id: &str, check: Option<(&str, bool)>) -> Bucket {
        Bucket {
            id: id.into(),
            name: id.into(),
            endpoint: "https://s3.example.com".into(),
            region: "us-east-1".into(),
            addressing: crate::config::Addressing::Path,
            allow_private_endpoint: false,
            bucket: "b".into(),
            test_file: "f".into(),
            check: check.map(|(o, ok)| BucketCheck {
                outcome: o.into(),
                ok,
                message: "m".into(),
                checked_at: String::new(),
            }),
        }
    }

    #[test]
    fn problems_follow_the_bell_rules() {
        let p = Panel {
            domains: vec![
                domain("ok.it", true, true, &[]),
                domain("new.it", false, false, &["dns"]),
                domain("lost.it", false, true, &["dns"]),
                domain("noreach.it", false, true, &["reach"]),
            ],
            buckets: vec![
                bucket("nocheck", None),
                bucket("down", Some(("unreachable", false))),
                bucket("auth", Some(("auth", false))),
                bucket("nofile", Some(("not_found", true))),
                bucket("fine", Some(("found", true))),
            ],
            ..Panel::default()
        };
        let got: Vec<(String, Level)> = problems(&p, &[], 0)
            .into_iter()
            .map(|x| (x.id, x.level))
            .collect();
        let want = [
            ("d:new.it", Level::Warn),
            ("d:lost.it", Level::Error),
            ("d:noreach.it", Level::Error),
            ("b:nocheck", Level::Warn),
            ("b:down", Level::Error),
            ("b:auth", Level::Error),
            ("b:nofile", Level::Warn),
        ];
        assert_eq!(got, want.map(|(i, l)| (i.to_string(), l)));
        let noreach = problems(&p, &[], 0)
            .into_iter()
            .find(|x| x.id == "d:noreach.it")
            .unwrap();
        assert!(noreach.title.starts_with("Il nodo non risponde"));
    }

    #[test]
    fn certificate_problems() {
        use crate::tls::{CertError, CertInfo};
        let mut p = Panel {
            domains: vec![domain("cdn.example.com", true, true, &[])],
            ..Panel::default()
        };
        let now = 1_000_000_000;
        let info = |na: Option<u64>, err: Option<&str>| CertInfo {
            host: "cdn.example.com".into(),
            not_after: na,
            issued_at: na.map(|_| 1),
            error: err.map(|m| CertError {
                at: now,
                message: m.into(),
            }),
        };
        // certificati automatici spenti: nessun avviso
        assert!(problems(&p, &[info(Some(now - 1), None)], now).is_empty());
        p.settings.acme.enabled = true;
        let one = |i: CertInfo| problems(&p, &[i], now);
        assert!(one(info(Some(now + 40 * 86400), None)).is_empty(), "valido");
        assert!(
            one(info(None, None)).is_empty(),
            "non ancora emesso, nessun errore: si aspetta"
        );
        let expiring = one(info(Some(now + 5 * 86400), None));
        assert_eq!(
            (expiring[0].id.as_str(), expiring[0].level),
            ("c:cdn.example.com", Level::Warn)
        );
        assert_eq!(one(info(Some(now - 10), None))[0].level, Level::Error);
        let failed = one(info(None, Some("la CA ha rifiutato")));
        assert_eq!(failed[0].level, Level::Warn);
        assert!(failed[0].text.contains("rifiutato"));
        // domini non certificabili (locali) e non verificati non generano avvisi
        p.domains = vec![domain("img.localhost", true, true, &[])];
        assert!(problems(&p, &[], now).is_empty());
    }

    fn prob(id: &str, level: Level) -> Problem {
        Problem {
            id: id.into(),
            level,
            title: format!("t {id}"),
            text: "x".into(),
        }
    }

    fn cfg(delay_min: u64) -> Config {
        Config {
            delay_min,
            ..Config::default()
        }
    }

    #[test]
    fn waits_then_notifies_once_then_recovers() {
        let mut st = State::default();
        let c = cfg(10);
        let p = [prob("d:a", Level::Error)];
        assert!(
            plan(&mut st, &p, &c, 1000).is_empty(),
            "prima dell'attesa niente"
        );
        assert!(plan(&mut st, &p, &c, 1000 + 599).is_empty());
        let ev = plan(&mut st, &p, &c, 1000 + 600);
        assert_eq!(ev.len(), 1);
        assert_eq!((ev[0].kind, ev[0].level), (Kind::Problem, Level::Error));
        assert!(
            plan(&mut st, &p, &c, 1000 + 5000).is_empty(),
            "una sola volta"
        );
        let ev = plan(&mut st, &[], &c, 1000 + 6000);
        assert_eq!(ev.len(), 1);
        assert_eq!((ev[0].kind, ev[0].level), (Kind::Recovered, Level::Error));
        assert!(plan(&mut st, &[], &c, 1000 + 7000).is_empty());
    }

    #[test]
    fn a_problem_that_clears_before_the_delay_is_silent() {
        let mut st = State::default();
        let c = cfg(10);
        plan(&mut st, &[prob("d:a", Level::Warn)], &c, 0);
        assert!(
            plan(&mut st, &[], &c, 60).is_empty(),
            "mai notificato: niente ripristino"
        );
    }

    #[test]
    fn worsening_notifies_again_after_a_new_wait() {
        let mut st = State::default();
        let c = cfg(1);
        plan(&mut st, &[prob("d:a", Level::Warn)], &c, 0);
        assert_eq!(
            plan(&mut st, &[prob("d:a", Level::Warn)], &c, 60)[0].level,
            Level::Warn
        );
        assert!(
            plan(&mut st, &[prob("d:a", Level::Error)], &c, 100).is_empty(),
            "riparte l'attesa"
        );
        let ev = plan(&mut st, &[prob("d:a", Level::Error)], &c, 160);
        assert_eq!((ev[0].kind, ev[0].level), (Kind::Problem, Level::Error));
        // torna avviso: già notificato più in alto, nessun nuovo messaggio
        assert!(plan(&mut st, &[prob("d:a", Level::Warn)], &c, 500).is_empty());
    }

    #[test]
    fn reminders_and_recovery_switch() {
        let mut st = State::default();
        let c = Config {
            delay_min: 0,
            reminder_hours: 2,
            recovery: false,
            ..Config::default()
        };
        let p = [prob("b:x", Level::Error)];
        assert_eq!(plan(&mut st, &p, &c, 0)[0].kind, Kind::Problem);
        assert!(plan(&mut st, &p, &c, 7199).is_empty());
        assert_eq!(plan(&mut st, &p, &c, 7200)[0].kind, Kind::Reminder);
        assert!(
            plan(&mut st, &[], &c, 7300).is_empty(),
            "ripristino disattivato"
        );
    }

    #[test]
    fn threshold_filters_levels() {
        assert!(
            Threshold::Warnings.accepts(Level::Warn) && Threshold::Warnings.accepts(Level::Error)
        );
        assert!(!Threshold::Errors.accepts(Level::Warn) && Threshold::Errors.accepts(Level::Error));
    }

    #[test]
    fn config_validation() {
        let mut c = Config::default();
        assert!(c.validate().is_ok(), "tutto spento è valido");
        c.telegram.enabled = true;
        assert!(c.validate().is_err(), "serve una chat");
        c.telegram.chat_ids = vec!["-1001234".into(), "@mio_canale".into()];
        assert!(c.validate().is_ok());
        c.telegram.chat_ids = vec!["abc".into()];
        assert!(c.validate().is_err());
        c.telegram.chat_ids = vec![];
        c.telegram.enabled = false;
        c.email = EmailCfg {
            enabled: true,
            host: "https://smtp.x.it".into(),
            from: "a@x.it".into(),
            to: vec!["b@x.it".into()],
            ..EmailCfg::default()
        };
        assert!(c.validate().unwrap_err().contains("server SMTP"));
        c.email.host = "smtp.x.it".into();
        assert!(c.validate().is_ok());
        c.email.to = vec!["non-una-mail".into()];
        assert!(c.validate().is_err());
        c.email.to = vec!["b@x.it\r\nBcc: e@x.it".into()];
        assert!(c.validate().is_err(), "niente iniezione di header");
        c.email.to = vec![];
        assert!(c.validate().is_err());
    }

    #[test]
    fn message_format() {
        let e = Event {
            kind: Kind::Problem,
            level: Level::Error,
            title: "Il dominio non risolve · a.it".into(),
            text: "dettaglio".into(),
        };
        let (s, b) = format_message(&e, "nodo abcd1234", None);
        assert_eq!(s, "[OtterRoute · errore] Il dominio non risolve · a.it");
        assert!(b.contains("dettaglio") && b.contains("nodo abcd1234") && !b.contains("Pannello"));
        let (_, b) = format_message(&e, "n", Some("https://p.example.com"));
        assert!(b.contains("Pannello: https://p.example.com"));
    }

    #[test]
    fn secrets_are_private_and_config_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let s = Secrets {
            smtp_password: "pw".into(),
            telegram_token: "tok".into(),
        };
        save_secrets(dir.path(), &s).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(secrets_path(dir.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(load_secrets(dir.path()).telegram_token, "tok");
        let c = Config {
            delay_min: 3,
            ..Config::default()
        };
        save_config(dir.path(), &c).unwrap();
        assert_eq!(load_config(dir.path()).unwrap().delay_min, 3);
        assert!(!std::fs::read_to_string(config_path(dir.path()))
            .unwrap()
            .contains("pw"));
    }

    /// Finto Telegram: registra le richieste e risponde come l'API vera.
    async fn fake_telegram(status: u16) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = l.accept().await else {
                    return;
                };
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                s2.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                let body = if status == 200 {
                    r#"{"ok":true}"#
                } else {
                    r#"{"ok":false,"description":"Bad Request: chat not found"}"#
                };
                let resp = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                let _ = sock.write_all(resp.as_bytes()).await;
            }
        });
        (base, seen)
    }

    #[tokio::test]
    async fn telegram_sends_to_every_chat_and_hides_the_token() {
        let (base, seen) = fake_telegram(200).await;
        let cfg = TelegramCfg {
            enabled: true,
            threshold: Threshold::Errors,
            chat_ids: vec!["1".into(), "@canale_x".into()],
        };
        telegram::send(&base, &cfg, "SEGRETO", "ciao")
            .await
            .unwrap();
        let reqs = seen.lock().unwrap().clone();
        assert_eq!(reqs.len(), 2);
        assert!(reqs[0].starts_with("POST /botSEGRETO/sendMessage"));
        assert!(reqs[0].contains("\"chat_id\":\"1\"") && reqs[0].contains("ciao"));
        assert!(reqs[1].contains("@canale_x"));

        let (base, _) = fake_telegram(400).await;
        let err = telegram::send(&base, &cfg, "SEGRETO", "x")
            .await
            .unwrap_err();
        assert!(err.contains("chat not found") && !err.contains("SEGRETO"));
        // server irraggiungibile: l'errore non contiene il token
        let err = telegram::send("http://127.0.0.1:1", &cfg, "SEGRETO", "x")
            .await
            .unwrap_err();
        assert!(!err.contains("SEGRETO"), "{err}");
        assert!(telegram::send(&base, &cfg, "", "x").await.is_err());
    }

    /// Finto server SMTP minimale (senza TLS): registra mittente, destinatari e corpo.
    async fn fake_smtp() -> (u16, std::sync::Arc<Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = l.accept().await else {
                    return;
                };
                let (r, mut w) = sock.into_split();
                let mut r = BufReader::new(r);
                let _ = w.write_all(b"220 fake ESMTP\r\n").await;
                let mut in_data = false;
                let mut line = String::new();
                loop {
                    line.clear();
                    if r.read_line(&mut line).await.unwrap_or(0) == 0 {
                        break;
                    }
                    let l = line.trim_end().to_string();
                    if in_data {
                        if l == "." {
                            in_data = false;
                            let _ = w.write_all(b"250 queued\r\n").await;
                        } else {
                            s2.lock().unwrap().push(l);
                        }
                        continue;
                    }
                    s2.lock().unwrap().push(l.clone());
                    let up = l.to_ascii_uppercase();
                    let reply: &[u8] = if up.starts_with("EHLO") {
                        b"250 fake\r\n"
                    } else if up == "DATA" {
                        in_data = true;
                        b"354 go\r\n"
                    } else if up == "QUIT" {
                        let _ = w.write_all(b"221 bye\r\n").await;
                        break;
                    } else {
                        b"250 ok\r\n"
                    };
                    let _ = w.write_all(reply).await;
                }
            }
        });
        (port, seen)
    }

    #[tokio::test]
    async fn email_reaches_all_recipients() {
        let (port, seen) = fake_smtp().await;
        let cfg = EmailCfg {
            enabled: true,
            threshold: Threshold::Errors,
            host: "127.0.0.1".into(),
            port,
            security: Security::None,
            user: String::new(),
            from: "OtterRoute <otter@example.com>".into(),
            to: vec!["a@example.com".into(), "b@example.com".into()],
        };
        email::send(
            &cfg,
            "",
            "[OtterRoute · errore] prova",
            "corpo del messaggio",
        )
        .await
        .unwrap();
        let all = seen.lock().unwrap().join("\n");
        assert!(all.contains("MAIL FROM:<otter@example.com>"), "{all}");
        assert!(all.contains("RCPT TO:<a@example.com>") && all.contains("RCPT TO:<b@example.com>"));
        assert!(all.contains("Subject:") && all.contains("corpo del messaggio"));
        // server giù: errore, non panic
        let bad = EmailCfg { port: 1, ..cfg };
        assert!(email::send(&bad, "", "s", "b").await.is_err());
    }

    /// TLS e STARTTLS verso un server che non li parla: errore pulito, senza panic
    /// (verifica anche che il provider crittografico di rustls sia disponibile).
    #[tokio::test]
    async fn tls_modes_fail_cleanly() {
        let (port, _) = fake_smtp().await;
        for security in [Security::Tls, Security::Starttls] {
            let cfg = EmailCfg {
                enabled: true,
                threshold: Threshold::Errors,
                host: "localhost".into(),
                port,
                security,
                user: "u".into(),
                from: "a@example.com".into(),
                to: vec!["b@example.com".into()],
            };
            let r =
                tokio::time::timeout(Duration::from_secs(30), email::send(&cfg, "pw", "s", "b"))
                    .await;
            assert!(r.expect("nessun blocco").is_err());
        }
    }

    #[tokio::test]
    async fn tick_notifies_and_logs_failures() {
        let dir = tempfile::tempdir().unwrap();
        let p = Panel {
            domains: vec![domain("lost.it", false, true, &["dns"])],
            ..Panel::default()
        };
        panel::save(dir.path(), &p).unwrap();
        let (base, seen) = fake_telegram(200).await;
        let c = Config {
            delay_min: 0,
            telegram: TelegramCfg {
                enabled: true,
                threshold: Threshold::Errors,
                chat_ids: vec!["7".into()],
            },
            ..Config::default()
        };
        save_config(dir.path(), &c).unwrap();
        save_secrets(
            dir.path(),
            &Secrets {
                telegram_token: "T".into(),
                ..Secrets::default()
            },
        )
        .unwrap();
        let mut n = Notifier::new(dir.path().to_path_buf(), "node-1234567890".into(), None);
        n.telegram_base = base;
        n.tick(100).await;
        n.tick(130).await;
        assert_eq!(seen.lock().unwrap().len(), 1, "una sola notifica");
        assert!(seen.lock().unwrap()[0].contains("Il dominio non risolve"));
        assert_eq!(n.log().len(), 1);
        assert!(n.log()[0].ok);
        // il dominio rientra: messaggio di ripristino
        panel::save(dir.path(), &Panel::default()).unwrap();
        n.tick(200).await;
        assert_eq!(seen.lock().unwrap().len(), 2);
        assert!(seen.lock().unwrap()[1].contains("ripristinato"));
        // lo stato sopravvive al riavvio: niente doppio invio
        panel::save(dir.path(), &p).unwrap();
        n.tick(300).await;
        let n2 = Notifier::new(dir.path().to_path_buf(), "x".into(), None);
        assert_eq!(n2.log().len(), 3);
        assert!(n.failing_channels().is_empty());
    }

    #[tokio::test]
    async fn failing_channel_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let c = Config {
            telegram: TelegramCfg {
                enabled: true,
                threshold: Threshold::Errors,
                chat_ids: vec!["7".into()],
            },
            ..Config::default()
        };
        save_config(dir.path(), &c).unwrap();
        let mut n = Notifier::new(dir.path().to_path_buf(), "n".into(), None);
        n.telegram_base = "http://127.0.0.1:1".into();
        assert!(n.test("telegram").await.is_err());
        assert_eq!(n.failing_channels(), vec!["telegram".to_string()]);
        assert!(
            n.test("telegram").await.unwrap_err().contains("attendi"),
            "limite di frequenza"
        );
    }
}
