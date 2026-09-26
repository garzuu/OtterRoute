//! Configurazione pubblicata: formato YAML, validazione e snapshot immutabile
//! usato dal gateway. Il gateway non parla mai con il database: riceve (per ora
//! da file) una configurazione completa, la valida e la sostituisce in blocco.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::s3;

// ---------------------------------------------------------------------------
// Formato grezzo (quello scritto dal controller / dall'amministratore)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    pub version: u64,
    #[serde(default)]
    pub storages: Vec<RawStorage>,
    #[serde(default)]
    pub destinations: Vec<RawDestination>,
    #[serde(default)]
    pub cache_policies: Vec<RawCachePolicy>,
    #[serde(default)]
    pub routes: Vec<RawRoute>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Addressing {
    /// `https://endpoint/bucket/key` (MinIO, Garage, SeaweedFS, ...)
    Path,
    /// `https://bucket.endpoint/key` (AWS, Wasabi, ...)
    Virtual,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawStorage {
    pub id: String,
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_addressing")]
    pub addressing: Addressing,
    pub credentials: RawCredentials,
    #[serde(default)]
    pub allow_private_endpoint: bool,
}

/// Le credenziali arrivano da variabili d'ambiente (`access_key_env` +
/// `secret_key_env`) oppure da un file JSON `{"access_key", "secret_key"}`
/// (`secret_file`), che è quello che scrive il pannello di onboarding.
/// Nell'MVP diventeranno un riferimento a un segreto cifrato gestito dal controller.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCredentials {
    #[serde(default)]
    pub access_key_env: Option<String>,
    #[serde(default)]
    pub secret_key_env: Option<String>,
    #[serde(default)]
    pub secret_file: Option<String>,
}

#[derive(Deserialize)]
struct SecretFile {
    access_key: String,
    secret_key: String,
}

/// Coppia (access key, secret key) risolta dalla configurazione.
fn resolve_credentials(
    c: &RawCredentials,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(String, String), String> {
    let non_empty = |v: Option<String>| v.filter(|v| !v.is_empty());
    match (&c.secret_file, &c.access_key_env, &c.secret_key_env) {
        (Some(_), None, None) => {}
        (None, Some(_), Some(_)) => {}
        (Some(_), _, _) => {
            return Err(
                "credenziali: usa secret_file oppure le variabili d'ambiente, non entrambi".into(),
            )
        }
        _ => {
            return Err(
                "credenziali: indica secret_file oppure access_key_env + secret_key_env".into(),
            )
        }
    }
    if let Some(path) = &c.secret_file {
        let raw = std::fs::read(path)
            .map_err(|e| format!("credenziali: impossibile leggere {path}: {e}"))?;
        let f: SecretFile = serde_json::from_slice(&raw)
            .map_err(|e| format!("credenziali: {path} non valido: {e}"))?;
        if f.access_key.is_empty() || f.secret_key.is_empty() {
            return Err(format!("credenziali: {path} contiene chiavi vuote"));
        }
        return Ok((f.access_key, f.secret_key));
    }
    let (ak_env, sk_env) = (
        c.access_key_env.as_deref().unwrap_or_default(),
        c.secret_key_env.as_deref().unwrap_or_default(),
    );
    match (non_empty(env(ak_env)), non_empty(env(sk_env))) {
        (Some(a), Some(s)) => Ok((a, s)),
        _ => Err(format!(
            "credenziali mancanti (variabili {ak_env} / {sk_env})"
        )),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDestination {
    pub id: String,
    pub storage: String,
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub cache_generation: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCachePolicy {
    pub id: String,
    #[serde(with = "crate::duration::serde")]
    pub ttl: Duration,
    #[serde(with = "crate::duration::serde", default = "default_ttl_not_found")]
    pub ttl_not_found: Duration,
    #[serde(default)]
    pub query_keys: Vec<String>,
    #[serde(with = "crate::duration::serde", default)]
    pub serve_stale_on_error: Duration,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRoute {
    pub id: String,
    pub host: String,
    pub path_prefix: String,
    #[serde(default = "default_true")]
    pub strip_prefix: bool,
    pub destination: String,
    pub cache_policy: String,
    /// richiede un link firmato con scadenza (`?exp=&sig=`)
    #[serde(default)]
    pub signed_urls: bool,
    /// trasforma le immagini al volo (`?w=&h=&fit=&fmt=&q=`)
    #[serde(default)]
    pub image_transform: bool,
}

fn default_region() -> String {
    "us-east-1".into()
}
fn default_addressing() -> Addressing {
    Addressing::Path
}
fn default_ttl_not_found() -> Duration {
    Duration::from_secs(60)
}
fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Snapshot validato
// ---------------------------------------------------------------------------

pub struct Storage {
    pub id: String,
    pub endpoint: Url,
    pub region: String,
    pub addressing: Addressing,
    pub access_key: String,
    pub secret_key: String,
    pub client: reqwest::Client,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // mai stampare le credenziali
        f.debug_struct("Storage")
            .field("id", &self.id)
            .field("endpoint", &self.endpoint.as_str())
            .field("region", &self.region)
            .field("addressing", &self.addressing)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct Destination {
    pub id: String,
    pub storage: Arc<Storage>,
    pub bucket: String,
    /// "" oppure "cartella/sotto/" (senza / iniziale, con / finale)
    pub prefix: String,
    pub revision: u64,
    pub cache_generation: u64,
}

#[derive(Debug, Clone)]
pub struct CachePolicy {
    pub id: String,
    pub ttl: Duration,
    pub ttl_not_found: Duration,
    pub query_keys: Vec<String>,
    pub serve_stale_on_error: Duration,
}

#[derive(Debug)]
pub struct Route {
    pub id: String,
    pub host: String,
    /// "/" oppure "/docs/" (sempre con / iniziale e finale)
    pub path_prefix: String,
    pub strip_prefix: bool,
    pub signed: bool,
    pub images: bool,
    pub dest: Destination,
    pub policy: CachePolicy,
}

#[derive(Debug)]
pub struct Snapshot {
    pub version: u64,
    /// per ogni host, regole ordinate dal prefisso più lungo al più corto
    pub routes_by_host: HashMap<String, Vec<Arc<Route>>>,
}

impl Snapshot {
    /// Nessuna regola: lo stato di un nodo appena installato, prima dell'onboarding.
    pub fn empty() -> Self {
        Snapshot {
            version: 0,
            routes_by_host: HashMap::new(),
        }
    }

    pub fn route_count(&self) -> usize {
        self.routes_by_host.values().map(Vec::len).sum()
    }
}

/// Legge e valida una configurazione. Gli errori sono raccolti tutti insieme,
/// così la UI (o chi legge il log) vede subito l'elenco completo.
pub fn parse(raw: &[u8]) -> anyhow::Result<Snapshot> {
    let rc: RawConfig = serde_yaml::from_slice(raw)?;
    validate(rc, &|k| std::env::var(k).ok())
}

pub fn validate(rc: RawConfig, env: &dyn Fn(&str) -> Option<String>) -> anyhow::Result<Snapshot> {
    let mut errs: Vec<String> = Vec::new();

    // --- storage ---
    let mut storages: HashMap<String, Arc<Storage>> = HashMap::new();
    for s in rc.storages {
        let ctx = format!("storage '{}'", s.id);
        if !valid_id(&s.id) {
            errs.push(format!("{ctx}: id non valido"));
            continue;
        }
        if storages.contains_key(&s.id) {
            errs.push(format!("{ctx}: id duplicato"));
            continue;
        }
        let endpoint = match check_endpoint(&s.endpoint, s.allow_private_endpoint) {
            Ok(u) => u,
            Err(e) => {
                errs.push(format!("{ctx}: {e}"));
                continue;
            }
        };
        if s.region.trim().is_empty() {
            errs.push(format!("{ctx}: region vuota"));
        }
        let (access_key, secret_key) = match resolve_credentials(&s.credentials, env) {
            Ok(c) => c,
            Err(e) => {
                errs.push(format!("{ctx}: {e}"));
                continue;
            }
        };
        let client = match s3::build_client(s.allow_private_endpoint) {
            Ok(c) => c,
            Err(e) => {
                errs.push(format!("{ctx}: client HTTP: {e}"));
                continue;
            }
        };
        storages.insert(
            s.id.clone(),
            Arc::new(Storage {
                id: s.id,
                endpoint,
                region: s.region,
                addressing: s.addressing,
                access_key,
                secret_key,
                client,
            }),
        );
    }

    // --- destinazioni ---
    let mut dests: HashMap<String, Destination> = HashMap::new();
    for d in rc.destinations {
        let ctx = format!("destination '{}'", d.id);
        if !valid_id(&d.id) {
            errs.push(format!("{ctx}: id non valido"));
            continue;
        }
        if dests.contains_key(&d.id) {
            errs.push(format!("{ctx}: id duplicato"));
            continue;
        }
        let Some(storage) = storages.get(&d.storage) else {
            errs.push(format!(
                "{ctx}: storage '{}' inesistente o non valido",
                d.storage
            ));
            continue;
        };
        if d.bucket.is_empty() || d.bucket.contains('/') || d.bucket.len() > 63 {
            errs.push(format!("{ctx}: bucket '{}' non valido", d.bucket));
            continue;
        }
        let prefix = match normalize_bucket_prefix(&d.prefix) {
            Ok(p) => p,
            Err(e) => {
                errs.push(format!("{ctx}: prefix: {e}"));
                continue;
            }
        };
        dests.insert(
            d.id.clone(),
            Destination {
                id: d.id,
                storage: storage.clone(),
                bucket: d.bucket,
                prefix,
                revision: d.revision,
                cache_generation: d.cache_generation,
            },
        );
    }

    // --- politiche cache ---
    let mut policies: HashMap<String, CachePolicy> = HashMap::new();
    for p in rc.cache_policies {
        let ctx = format!("cache_policy '{}'", p.id);
        if !valid_id(&p.id) {
            errs.push(format!("{ctx}: id non valido"));
            continue;
        }
        if policies.contains_key(&p.id) {
            errs.push(format!("{ctx}: id duplicato"));
            continue;
        }
        let mut qk = p.query_keys.clone();
        qk.sort();
        qk.dedup();
        policies.insert(
            p.id.clone(),
            CachePolicy {
                id: p.id,
                ttl: p.ttl,
                ttl_not_found: p.ttl_not_found,
                query_keys: qk,
                serve_stale_on_error: p.serve_stale_on_error,
            },
        );
    }

    // --- regole ---
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut seen_match: HashMap<(String, String), String> = HashMap::new();
    let mut routes_by_host: HashMap<String, Vec<Arc<Route>>> = HashMap::new();
    for r in rc.routes {
        let ctx = format!("route '{}'", r.id);
        if !valid_id(&r.id) {
            errs.push(format!("{ctx}: id non valido"));
            continue;
        }
        if !seen_ids.insert(r.id.clone()) {
            errs.push(format!("{ctx}: id duplicato"));
            continue;
        }
        let host = match normalize_host(&r.host) {
            Ok(h) => h,
            Err(e) => {
                errs.push(format!("{ctx}: host: {e}"));
                continue;
            }
        };
        let prefix = match normalize_route_prefix(&r.path_prefix) {
            Ok(p) => p,
            Err(e) => {
                errs.push(format!("{ctx}: path_prefix: {e}"));
                continue;
            }
        };
        if let Some(other) = seen_match.get(&(host.clone(), prefix.clone())) {
            errs.push(format!(
                "{ctx}: {host}{prefix} è già assegnato alla regola '{other}'"
            ));
            continue;
        }
        let Some(dest) = dests.get(&r.destination) else {
            errs.push(format!(
                "{ctx}: destination '{}' inesistente o non valida",
                r.destination
            ));
            continue;
        };
        let Some(policy) = policies.get(&r.cache_policy) else {
            errs.push(format!(
                "{ctx}: cache_policy '{}' inesistente o non valida",
                r.cache_policy
            ));
            continue;
        };
        seen_match.insert((host.clone(), prefix.clone()), r.id.clone());
        routes_by_host
            .entry(host.clone())
            .or_default()
            .push(Arc::new(Route {
                id: r.id,
                host,
                path_prefix: prefix,
                strip_prefix: r.strip_prefix,
                signed: r.signed_urls,
                images: r.image_transform,
                dest: dest.clone(),
                policy: policy.clone(),
            }));
    }

    if !errs.is_empty() {
        anyhow::bail!("configurazione non valida:\n  - {}", errs.join("\n  - "));
    }

    for routes in routes_by_host.values_mut() {
        routes.sort_by_key(|r| std::cmp::Reverse(r.path_prefix.len()));
    }

    Ok(Snapshot {
        version: rc.version,
        routes_by_host,
    })
}

// ---------------------------------------------------------------------------
// Normalizzazioni e controlli
// ---------------------------------------------------------------------------

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Host pubblico: minuscolo, senza porta, senza punto finale, senza wildcard.
pub fn normalize_host(h: &str) -> Result<String, String> {
    let h = h.trim().trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() || h.len() > 253 {
        return Err("vuoto o troppo lungo".into());
    }
    if h.contains(':') {
        return Err("non indicare la porta".into());
    }
    if h.contains('*') {
        return Err("wildcard non supportate nell'MVP".into());
    }
    if !h
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        return Err("caratteri non ammessi".into());
    }
    Ok(h)
}

/// Prefisso della regola: "/" oppure "/a/b/".
pub fn normalize_route_prefix(p: &str) -> Result<String, String> {
    if !p.starts_with('/') {
        return Err("deve iniziare con /".into());
    }
    let segs: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    for s in &segs {
        if *s == "." || *s == ".." {
            return Err("segmenti . e .. non ammessi".into());
        }
        if s.contains('%') {
            return Err("usa il percorso decodificato, senza %".into());
        }
    }
    if p.contains("//") {
        return Err("// non ammesso".into());
    }
    if segs.is_empty() {
        Ok("/".into())
    } else {
        Ok(format!("/{}/", segs.join("/")))
    }
}

/// Cartella nel bucket: "" oppure "a/b/".
pub fn normalize_bucket_prefix(p: &str) -> Result<String, String> {
    let segs: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    for s in &segs {
        if *s == "." || *s == ".." {
            return Err("segmenti . e .. non ammessi".into());
        }
    }
    if segs.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{}/", segs.join("/")))
    }
}

pub fn check_endpoint(raw: &str, allow_private: bool) -> Result<Url, String> {
    let u = Url::parse(raw).map_err(|e| format!("endpoint non valido: {e}"))?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return Err("endpoint: schema ammesso solo http o https".into());
    }
    if !(u.path() == "/" || u.path().is_empty()) || u.query().is_some() || u.fragment().is_some() {
        return Err("endpoint: niente percorso, query o frammento".into());
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("endpoint: niente credenziali nell'URL".into());
    }
    let host = u.host_str().ok_or("endpoint senza host")?;
    if !allow_private {
        let h = host.trim_start_matches('[').trim_end_matches(']');
        let private_name = h.eq_ignore_ascii_case("localhost")
            || h.ends_with(".localhost")
            || h.ends_with(".local")
            || h.ends_with(".internal");
        let private_ip = h.parse::<IpAddr>().map(s3::is_private_ip).unwrap_or(false);
        if private_name || private_ip {
            return Err(format!(
                "endpoint interno '{host}': serve allow_private_endpoint: true"
            ));
        }
    }
    Ok(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(k: &str) -> Option<String> {
        Some(format!("val-{k}"))
    }

    const BASE: &str = r#"
version: 1
storages:
  - id: st_a
    endpoint: https://s3.example.com
    credentials: { access_key_env: A, secret_key_env: B }
destinations:
  - { id: d1, storage: st_a, bucket: catalogo, prefix: /foto }
cache_policies:
  - { id: cp, ttl: 1h }
routes:
  - { id: r1, host: IMG.Azienda.it., path_prefix: /, destination: d1, cache_policy: cp }
  - { id: r2, host: img.azienda.it, path_prefix: /docs, destination: d1, cache_policy: cp }
"#;

    fn parse_with(y: &str) -> anyhow::Result<Snapshot> {
        validate(serde_yaml::from_str(y).unwrap(), &env)
    }

    #[test]
    fn valid_config_is_normalized_and_sorted() {
        let s = parse_with(BASE).unwrap();
        let rs = &s.routes_by_host["img.azienda.it"];
        assert_eq!(rs[0].path_prefix, "/docs/");
        assert_eq!(rs[1].path_prefix, "/");
        assert_eq!(rs[0].dest.prefix, "foto/");
        assert_eq!(rs[0].policy.ttl_not_found, Duration::from_secs(60));
    }

    #[test]
    fn duplicate_match_is_rejected() {
        let y = BASE.replace("path_prefix: /docs", "path_prefix: /");
        let e = parse_with(&y).unwrap_err().to_string();
        assert!(e.contains("già assegnato"), "{e}");
    }

    #[test]
    fn private_endpoint_requires_opt_in() {
        let y = BASE.replace("https://s3.example.com", "http://127.0.0.1:9000");
        assert!(parse_with(&y).is_err());
        let y = y.replace(
            "credentials: {",
            "allow_private_endpoint: true\n    credentials: {",
        );
        parse_with(&y).unwrap();
    }

    #[test]
    fn missing_references_are_all_reported() {
        let y = BASE
            .replace("storage: st_a", "storage: nope")
            .replace("cache_policy: cp }", "cache_policy: zzz }");
        let e = parse_with(&y).unwrap_err().to_string();
        assert!(e.contains("storage 'nope'"), "{e}");
        assert!(e.matches("destination 'd1'").count() >= 2, "{e}");
    }

    #[test]
    fn secret_file_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("st_a.json");
        std::fs::write(&f, r#"{"access_key":"AK","secret_key":"SK"}"#).unwrap();
        let y = BASE.replace(
            "credentials: { access_key_env: A, secret_key_env: B }",
            &format!("credentials: {{ secret_file: {} }}", f.display()),
        );
        parse_with(&y).unwrap();
        let both = BASE.replace(
            "secret_key_env: B",
            &format!("secret_key_env: B, secret_file: {}", f.display()),
        );
        assert!(parse_with(&both).is_err());
        let missing = y.replace("st_a.json", "nope.json");
        assert!(parse_with(&missing).is_err());
    }

    #[test]
    fn prefixes() {
        assert_eq!(normalize_route_prefix("/").unwrap(), "/");
        assert_eq!(normalize_route_prefix("/a/b").unwrap(), "/a/b/");
        assert!(normalize_route_prefix("a/").is_err());
        assert!(normalize_route_prefix("/a/../b").is_err());
        assert_eq!(normalize_bucket_prefix("").unwrap(), "");
        assert_eq!(normalize_bucket_prefix("/x/y/").unwrap(), "x/y/");
    }
}
