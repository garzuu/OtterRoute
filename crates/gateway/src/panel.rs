//! Stato gestito dal pannello (domini, bucket, instradamenti) e generazione della
//! configurazione del gateway. La configurazione è una funzione dello stato del
//! pannello: ogni modifica la rigenera per intero, così resta sempre coerente.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Addressing;
use crate::dns::{CheckResult, Stage};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Panel {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub domains: Vec<Domain>,
    #[serde(default)]
    pub buckets: Vec<Bucket>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

/// Porte standard del nodo. Il traffico pubblico usa HTTP 80 e HTTPS 443; il
/// pannello (9090) resta solo su localhost.
pub const DEFAULT_HTTP_PORT: u16 = 80;
pub const DEFAULT_HTTPS_PORT: u16 = 443;

/// Impostazioni modificabili dal pannello: vuoto = porta standard.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub http_port: Option<u16>,
    #[serde(default)]
    pub https_port: Option<u16>,
}

impl Settings {
    /// Porta su cui i domini arrivano al nodo in HTTP: è quella che verifica il controllo.
    pub fn http(&self) -> u16 {
        self.http_port.unwrap_or(DEFAULT_HTTP_PORT)
    }
    pub fn https(&self) -> u16 {
        self.https_port.unwrap_or(DEFAULT_HTTPS_PORT)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub host: String,
    pub verified: bool,
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub records: Vec<String>,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub stages: Vec<Stage>,
    /// è già stato verificato almeno una volta: se ora fallisce è un errore,
    /// non una semplice attesa della propagazione DNS
    #[serde(default)]
    pub ever_verified: bool,
    /// da quando lo stato (verificato / non verificato) è quello attuale
    #[serde(default)]
    pub since: Option<String>,
}

impl Domain {
    pub fn apply(&mut self, r: CheckResult) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        if self.since.is_none() || self.verified != r.ok {
            self.since = Some(now.clone());
        }
        self.verified = r.ok;
        self.ever_verified |= r.ok;
        self.records = r.records;
        self.message = r.message;
        self.stages = r.stages;
        self.checked_at = Some(now);
    }
}

/// Uno storage con le sue credenziali e il bucket da cui si legge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bucket {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub region: String,
    pub addressing: Addressing,
    pub allow_private_endpoint: bool,
    pub bucket: String,
    /// file letto per verificare la connessione
    #[serde(default)]
    pub test_file: String,
    #[serde(default)]
    pub check: Option<BucketCheck>,
}

/// Esito dell'ultima verifica di lettura dello storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketCheck {
    /// found, not_found, auth, unreachable, invalid
    pub outcome: String,
    pub ok: bool,
    pub message: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub domain: String,
    pub path_prefix: String,
    pub bucket_id: String,
    #[serde(default)]
    pub folder: String,
    /// aumentarlo rende irraggiungibili le copie in cache dell'instradamento
    #[serde(default = "first_generation")]
    pub cache_generation: u64,
}

fn first_generation() -> u64 {
    1
}

pub fn panel_path(state_dir: &Path) -> PathBuf {
    state_dir.join("panel.json")
}

pub fn secrets_dir(state_dir: &Path) -> std::io::Result<PathBuf> {
    std::path::absolute(state_dir.join("secrets"))
}

pub fn load(state_dir: &Path) -> Result<Panel, String> {
    match std::fs::read(panel_path(state_dir)) {
        Ok(raw) => serde_json::from_slice(&raw).map_err(|e| format!("panel.json non valido: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Panel::default()),
        Err(e) => Err(format!("lettura panel.json: {e}")),
    }
}

pub fn save(state_dir: &Path, panel: &Panel) -> std::io::Result<()> {
    let data = serde_json::to_vec_pretty(panel).map_err(std::io::Error::other)?;
    crate::admin::write_atomic(&panel_path(state_dir), &data, false)
}

/// Configurazione del gateway per lo stato del pannello. Ogni instradamento ha
/// la propria destinazione (bucket + cartella); tutti condividono la politica di
/// cache "standard".
pub fn generate_config(panel: &Panel, version: u64, secrets: &Path) -> Value {
    let storages: Vec<Value> = panel
        .buckets
        .iter()
        .map(|b| {
            json!({
                "id": b.id,
                "endpoint": b.endpoint,
                "region": b.region,
                "addressing": match b.addressing { Addressing::Path => "path", Addressing::Virtual => "virtual" },
                "allow_private_endpoint": b.allow_private_endpoint,
                "credentials": { "secret_file": secrets.join(format!("{}.json", b.id)).to_string_lossy() },
            })
        })
        .collect();
    let mut destinations = Vec::new();
    let mut routes = Vec::new();
    for r in &panel.rules {
        let Some(b) = panel.buckets.iter().find(|b| b.id == r.bucket_id) else {
            continue;
        };
        destinations.push(json!({
            "id": r.id,
            "storage": b.id,
            "bucket": b.bucket,
            "prefix": r.folder,
            "revision": 1,
            "cache_generation": r.cache_generation,
        }));
        routes.push(json!({
            "id": r.id,
            "host": r.domain,
            "path_prefix": r.path_prefix,
            "destination": r.id,
            "cache_policy": "standard",
        }));
    }
    json!({
        "version": version,
        "storages": storages,
        "destinations": destinations,
        "cache_policies": [{
            "id": "standard",
            "ttl": "1h",
            "ttl_not_found": "60s",
            "query_keys": [],
            "serve_stale_on_error": "24h",
        }],
        "routes": routes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_is_generated_from_panel() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("b1.json"),
            r#"{"access_key":"a","secret_key":"s"}"#,
        )
        .unwrap();
        let panel = Panel {
            buckets: vec![Bucket {
                id: "b1".into(),
                name: "Catalogo".into(),
                endpoint: "https://s3.example.com".into(),
                region: "us-east-1".into(),
                addressing: Addressing::Path,
                allow_private_endpoint: false,
                bucket: "catalogo".into(),
                test_file: String::new(),
                check: None,
            }],
            rules: vec![Rule {
                id: "r1".into(),
                domain: "img.example.com".into(),
                path_prefix: "/".into(),
                bucket_id: "b1".into(),
                folder: "foto/".into(),
                cache_generation: 4,
            }],
            ..Panel::default()
        };
        let yaml = serde_yaml::to_string(&generate_config(&panel, 3, dir.path())).unwrap();
        let snap = crate::config::parse(yaml.as_bytes()).unwrap();
        assert_eq!(snap.version, 3);
        assert_eq!(
            yaml.matches("cache_generation: 4").count(),
            1,
            "la generazione della regola arriva in configurazione"
        );
        assert_eq!(snap.route_count(), 1);
        assert_eq!(
            snap.routes_by_host["img.example.com"][0].dest.prefix,
            "foto/"
        );
    }

    fn result(ok: bool) -> CheckResult {
        CheckResult {
            ok,
            records: vec![],
            message: String::new(),
            stages: vec![],
        }
    }

    #[test]
    fn domain_states_pending_verified_error() {
        let mut d = Domain {
            host: "a.it".into(),
            verified: false,
            checked_at: None,
            records: vec![],
            message: String::new(),
            stages: vec![],
            ever_verified: false,
            since: None,
        };
        // mai verificato: resta "in attesa", non è un errore
        d.apply(result(false));
        assert!(!d.verified && !d.ever_verified);
        // diventa valido
        d.apply(result(true));
        assert!(d.verified && d.ever_verified);
        // poi smette di risolvere: era valido, quindi ora è un errore
        d.apply(result(false));
        assert!(!d.verified && d.ever_verified);
        assert!(d.since.is_some());
    }

    #[test]
    fn missing_panel_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().rules.is_empty());
    }
}
