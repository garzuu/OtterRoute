//! Porta di amministrazione: salute, stato, API del pannello (domini, bucket,
//! instradamenti) e file statici della sua build. In ascolto su 127.0.0.1 per
//! default: chi la raggiunge può cambiare la configurazione, quindi non va esposta.

// Le risposte HTTP sono usate come tipo d'errore dei controlli interni.
#![allow(clippy::result_large_err)]

use std::convert::Infallible;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use http::{header, HeaderValue, Method, Request, Response, StatusCode};
use http_body_util::{BodyExt, Limited};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::audit;
use crate::auth::{self, Auth, Login, Principal, Requirement, SecondFactorError};
use crate::body::{self, Body};
use crate::config::{self, Addressing};
use crate::dns;
use crate::handler::AppState;
use crate::panel::{self, Bucket, BucketCheck, Domain, Panel, Rule};
use crate::s3;

const MAX_BODY: usize = 64 * 1024;
pub struct Admin {
    pub state: Arc<AppState>,
    pub config_path: PathBuf,
    pub state_dir: PathBuf,
    pub ui_dir: PathBuf,
    /// guida costruita (VitePress), servita sotto `/docs/` se la cartella esiste
    pub docs_dir: Option<PathBuf>,
    /// indirizzo della guida online, per i link "Guida" del pannello (vuoto = nessun link)
    pub docs_url: String,
    /// porta su cui il nodo è effettivamente in ascolto (OTR_LISTEN)
    pub listen_port: u16,
    /// intervalli del controllo periodico dei domini (la propagazione DNS può
    /// richiedere fino a 48 ore; quelli verificati vanno tenuti d'occhio)
    pub recheck_pending: Duration,
    pub recheck_verified: Duration,
    /// una sola modifica alla configurazione alla volta
    pub write_lock: Mutex<()>,
    pub auth: Auth,
    pub notifier: Arc<crate::notify::Notifier>,
}

pub async fn handle(
    admin: Arc<Admin>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Body>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let principal = admin.auth.principal(cookie.as_deref());
    Ok(match (&method, path.as_str()) {
        (&Method::GET, "/healthz") => json(StatusCode::OK, json!({"status": "ok"})),
        (&Method::GET, "/status") => status(&admin),
        (&Method::GET, "/metrics") => prometheus(&admin),
        (&Method::GET, "/api/session") => session_info(&admin, principal.as_ref()),
        (&Method::POST, "/api/setup") => match read_json::<Credentials>(req).await {
            Ok(b) => setup(&admin, b).await,
            Err(r) => r,
        },
        (&Method::POST, "/api/login") => match read_json::<Credentials>(req).await {
            Ok(b) => login(&admin, b).await,
            Err(r) => r,
        },
        (&Method::POST, "/api/login/2fa") => match read_json::<TwoFactorLogin>(req).await {
            Ok(b) => login_two_factor(&admin, b).await,
            Err(r) => r,
        },
        (&Method::POST, "/api/logout") => {
            admin.auth.end_session(cookie.as_deref());
            with_cookie(json(StatusCode::OK, json!({"ok": true})), clear_cookie())
        }
        (_, p) if p.starts_with("/api/") => match principal {
            None => error(StatusCode::UNAUTHORIZED, "accesso richiesto"),
            // dentro lo scope le azioni si attribuiscono all'utente (registro attività)
            Some(pr) => {
                let who = pr.username.clone();
                audit::scope(who, api(&admin, &pr, &method, p, req)).await
            }
        },
        (&Method::GET | &Method::HEAD, p) if p == "/docs" || p.starts_with("/docs/") => {
            serve_docs(&admin, p)
        }
        (&Method::GET | &Method::HEAD, p) => serve_static(&admin, p),
        _ => json(StatusCode::NOT_FOUND, json!({"error": "not found"})),
    })
}

// ---------------------------------------------------------------------------
// Autorizzazione: quale scope serve per ogni endpoint
// ---------------------------------------------------------------------------

/// Cosa richiede un endpoint.
#[derive(Debug, PartialEq, Eq)]
pub enum Access {
    /// basta essere autenticati (il profilo personale, la lettura filtrata)
    Open,
    Scope(&'static str),
}

/// Tabella unica dei permessi. Un endpoint che non compare qui non esiste:
/// un test controlla che ogni route del codice sia coperta.
pub fn access(method: &Method, path: &str) -> Option<Access> {
    use Access::{Open, Scope};
    let m = method.as_str();
    Some(match (m, path) {
        ("GET", "/api/panel") => Open,
        ("GET", "/api/metrics") => Scope("metrics:read"),
        ("PUT", "/api/settings") => Scope("settings:write"),
        ("POST", "/api/domains" | "/api/domains/check" | "/api/domains/test") => {
            Scope("domains:write")
        }
        ("DELETE", p) if p.starts_with("/api/domains/") => Scope("domains:write"),
        ("POST", "/api/buckets" | "/api/buckets/check" | "/api/buckets/test") => {
            Scope("buckets:write")
        }
        ("DELETE", p) if p.starts_with("/api/buckets/") => Scope("buckets:write"),
        ("POST", "/api/rules") => Scope("routes:write"),
        ("DELETE", p) if p.starts_with("/api/rules/") => Scope("routes:write"),
        ("POST", "/api/probe") => Scope("routes:read"),
        (_, "/api/notifications" | "/api/notifications/test") => Scope("notifications:manage"),
        (_, "/api/users" | "/api/audit" | "/api/policy") => Scope("users:manage"),
        (_, p) if p.starts_with("/api/users/") => Scope("users:manage"),
        (_, "/api/me") => Open,
        (_, p) if p.starts_with("/api/me/") => Open,
        _ => return None,
    })
}

/// Finché l'utente deve cambiare la password o attivare la 2FA può fare solo quello.
fn allowed_while_limited(r: Requirement, method: &Method, path: &str) -> bool {
    match r {
        Requirement::ChangePassword => {
            path == "/api/me" && method == Method::GET
                || path == "/api/me/password" && method == Method::PUT
        }
        Requirement::SetupTwoFactor => {
            path == "/api/me" && method == Method::GET || path.starts_with("/api/me/2fa/")
        }
    }
}

/// API del pannello (richiede una sessione valida).
async fn api(
    admin: &Admin,
    pr: &Principal,
    method: &Method,
    path: &str,
    req: Request<hyper::body::Incoming>,
) -> Response<Body> {
    if let Some(r) = pr.requirement {
        if !allowed_while_limited(r, method, path) {
            return error(
                StatusCode::FORBIDDEN,
                match r {
                    Requirement::ChangePassword => "devi prima cambiare la password",
                    Requirement::SetupTwoFactor => {
                        "il criterio di sicurezza richiede di attivare la 2FA"
                    }
                },
            );
        }
    }
    match access(method, path) {
        Some(Access::Open) => {}
        Some(Access::Scope(scope)) => {
            if !pr.can(scope) {
                audit::log_as(&pr.username, "denied", &format!("{method} {path}"), false);
                return error(
                    StatusCode::FORBIDDEN,
                    format!("permesso negato: serve lo scope {scope}"),
                );
            }
        }
        None => return error(StatusCode::NOT_FOUND, "not found"),
    }
    if path == "/api/users"
        || path.starts_with("/api/users/")
        || path == "/api/me"
        || path.starts_with("/api/me/")
        || path == "/api/audit"
        || path == "/api/policy"
    {
        return crate::admin_users::handle(admin, pr, method, path, req).await;
    }
    macro_rules! with_body {
        ($t:ty, $f:expr) => {
            match read_json::<$t>(req).await {
                Ok(b) => $f(admin, b).await,
                Err(r) => r,
            }
        };
    }
    match (method, path) {
        (&Method::GET, "/api/panel") => panel_state(admin, pr),
        (&Method::GET, "/api/metrics") => metrics_api(admin, req.uri().query()),
        (&Method::PUT, "/api/settings") => with_body!(SettingsReq, save_settings),
        (&Method::POST, "/api/domains") => with_body!(HostReq, add_domain),
        (&Method::POST, "/api/domains/check") => with_body!(HostReq, check_domain),
        (&Method::POST, "/api/domains/test") => with_body!(HostReq, test_domain),
        (&Method::POST, "/api/buckets/check") => with_body!(IdReq, check_bucket_saved),
        (&Method::POST, "/api/buckets/test") => match read_json::<TestReq>(req).await {
            Ok(b) => test_storage(b).await,
            Err(r) => r,
        },
        (&Method::POST, "/api/buckets") => with_body!(BucketReq, add_bucket),
        (&Method::POST, "/api/rules") => with_body!(RuleReq, add_rule),
        (&Method::POST, "/api/probe") => with_body!(ProbeReq, probe),
        (&Method::GET, "/api/notifications") => notifications_get(admin),
        (&Method::PUT, "/api/notifications") => with_body!(NotifyReq, notifications_put),
        (&Method::POST, "/api/notifications/test") => with_body!(NotifyTestReq, notifications_test),
        (&Method::DELETE, p) if p.starts_with("/api/domains/") => {
            delete_domain(admin, &p["/api/domains/".len()..]).await
        }
        (&Method::DELETE, p) if p.starts_with("/api/buckets/") => {
            delete_bucket(admin, &p["/api/buckets/".len()..]).await
        }
        (&Method::DELETE, p) if p.starts_with("/api/rules/") => {
            delete_rule(admin, &p["/api/rules/".len()..]).await
        }
        _ => error(StatusCode::NOT_FOUND, "not found"),
    }
}

// ---------------------------------------------------------------------------
// Accesso
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

fn session_cookie(token: &str) -> String {
    format!(
        "{}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        auth::COOKIE,
        auth::SESSION_TTL.as_secs()
    )
}

fn clear_cookie() -> String {
    format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        auth::COOKIE
    )
}

fn with_cookie(mut r: Response<Body>, cookie: String) -> Response<Body> {
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        r.headers_mut().insert(header::SET_COOKIE, v);
    }
    r
}

impl Admin {
    /// Dove mandare chi chiede la guida: la copia offline se c'è, altrimenti l'URL configurato.
    fn docs_link(&self) -> Option<String> {
        if self
            .docs_dir
            .as_ref()
            .is_some_and(|d| d.join("index.html").is_file())
        {
            Some("/docs/".into())
        } else if self.docs_url.is_empty() {
            None
        } else {
            Some(self.docs_url.clone())
        }
    }
}

fn session_info(admin: &Admin, p: Option<&Principal>) -> Response<Body> {
    json(
        StatusCode::OK,
        json!({
            "setup_required": admin.auth.setup_required(),
            "authenticated": p.is_some(),
            "username": p.map(|p| p.username.clone()),
            "user": p.map(|p| json!({
                "username": p.username,
                "role": p.role,
                "scopes": p.scopes,
                "two_factor": p.two_factor,
                "requirement": p.requirement.map(|r| match r {
                    Requirement::ChangePassword => "change_password",
                    Requirement::SetupTwoFactor => "setup_2fa",
                }),
            })),
            "policy": admin.auth.users.policy(),
            "docs_url": admin.docs_link(),
        }),
    )
}

/// Primo accesso: crea l'amministratore e apre la sessione.
async fn setup(admin: &Admin, c: Credentials) -> Response<Body> {
    let _guard = admin.write_lock.lock().await;
    match admin
        .auth
        .create_first_admin(&c.username, &c.password)
        .await
    {
        Ok(user) => {
            let token = admin.auth.start_session(&user.id);
            tracing::info!(user = %user.username, "amministratore creato");
            audit::log_as(&user.username, "setup", "primo amministratore", true);
            with_cookie(
                json(StatusCode::OK, json!({"username": user.username})),
                session_cookie(&token),
            )
        }
        Err(e) => error(StatusCode::UNPROCESSABLE_ENTITY, e),
    }
}

fn too_many(secs: u64) -> Response<Body> {
    error(
        StatusCode::TOO_MANY_REQUESTS,
        format!("troppi tentativi: riprova tra {} minuti", secs.div_ceil(60)),
    )
}

async fn login(admin: &Admin, c: Credentials) -> Response<Body> {
    match admin.auth.login_password(&c.username, &c.password).await {
        Login::Session(token) => {
            audit::log_as(&c.username, "login", "", true);
            with_cookie(
                json(StatusCode::OK, json!({"ok": true})),
                session_cookie(&token),
            )
        }
        // password giusta ma serve il codice: nessun cookie finché non arriva
        Login::SecondFactor(challenge) => json(
            StatusCode::OK,
            json!({"needs_2fa": true, "challenge": challenge}),
        ),
        Login::Invalid => {
            audit::log_as(&c.username, "login", "credenziali errate", false);
            // rallenta i tentativi a raffica
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            error(StatusCode::UNAUTHORIZED, "nome utente o password errati")
        }
        Login::Locked(secs) => {
            audit::log_as(&c.username, "login", "bloccato", false);
            too_many(secs)
        }
    }
}

#[derive(Deserialize)]
struct TwoFactorLogin {
    challenge: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    recovery_code: Option<String>,
}

async fn login_two_factor(admin: &Admin, c: TwoFactorLogin) -> Response<Body> {
    match admin
        .auth
        .login_second_factor(&c.challenge, c.code.as_deref(), c.recovery_code.as_deref())
        .await
    {
        Ok(token) => {
            let who = admin
                .auth
                .principal(Some(&format!("{}={token}", auth::COOKIE)))
                .map_or_else(|| "-".into(), |p| p.username);
            audit::log_as(
                &who,
                "login",
                if c.recovery_code.is_some() {
                    "codice di recupero"
                } else {
                    "2FA"
                },
                true,
            );
            with_cookie(
                json(StatusCode::OK, json!({"ok": true})),
                session_cookie(&token),
            )
        }
        Err(SecondFactorError::Invalid) => {
            audit::log_as("-", "login-2fa", "codice errato", false);
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            error(StatusCode::UNAUTHORIZED, "codice errato")
        }
        Err(SecondFactorError::Expired) => error(
            StatusCode::UNAUTHORIZED,
            "la verifica è scaduta: inserisci di nuovo nome utente e password",
        ),
        Err(SecondFactorError::Locked(secs)) => too_many(secs),
    }
}

pub(crate) fn json(status: StatusCode, v: Value) -> Response<Body> {
    let mut r = Response::new(body::full(v.to_string()));
    *r.status_mut() = status;
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

pub(crate) fn error(status: StatusCode, msg: impl Into<String>) -> Response<Body> {
    json(status, json!({ "error": msg.into() }))
}

/// Solo `application/json`: un form o un `fetch` cross-origin "semplice" non può
/// modificare la configurazione (il browser chiederebbe prima il preflight).
#[allow(clippy::result_large_err)]
pub(crate) async fn read_json<T: DeserializeOwned>(
    req: Request<hyper::body::Incoming>,
) -> Result<T, Response<Body>> {
    let is_json = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if !is_json {
        return Err(error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "serve Content-Type: application/json",
        ));
    }
    let bytes = Limited::new(req.into_body(), MAX_BODY)
        .collect()
        .await
        .map_err(|_| error(StatusCode::PAYLOAD_TOO_LARGE, "richiesta troppo grande"))?
        .to_bytes();
    serde_json::from_slice(&bytes)
        .map_err(|e| error(StatusCode::BAD_REQUEST, format!("JSON non valido: {e}")))
}

// ---------------------------------------------------------------------------
// Stato
// ---------------------------------------------------------------------------

fn routes_json(state: &AppState) -> Vec<Value> {
    let snap = state.snapshot.load();
    let mut routes: Vec<_> = snap
        .routes_by_host
        .values()
        .flatten()
        .map(|r| {
            json!({
                "id": r.id,
                "match": format!("{}{}", r.host, r.path_prefix),
                "host": r.host,
                "path_prefix": r.path_prefix,
                "destination": r.dest.id,
                "cache_policy": r.policy.id,
                "storage": r.dest.storage.id,
                "endpoint": r.dest.storage.endpoint.as_str(),
                "bucket": r.dest.bucket,
                "prefix": r.dest.prefix,
                "cache_generation": r.dest.cache_generation,
            })
        })
        .collect();
    routes.sort_by(|a, b| a["match"].as_str().cmp(&b["match"].as_str()));
    routes
}

fn status(admin: &Admin) -> Response<Body> {
    let state = &admin.state;
    json(
        StatusCode::OK,
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "config_version": state.snapshot.load().version,
            "routes": routes_json(state),
            "cache": state.cache.stats(),
        }),
    )
}

/// Formato Prometheus, sulla porta admin (solo locale) e senza login, come `/status`.
fn prometheus(admin: &Admin) -> Response<Body> {
    let s = &admin.state;
    let text = s
        .metrics
        .render_prometheus(&s.cache.stats(), s.snapshot.load().version);
    let mut r = Response::new(body::full(text));
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// Statistiche di traffico per la dashboard: `?range=1h|24h|7d`.
fn metrics_api(admin: &Admin, query: Option<&str>) -> Response<Body> {
    let range = query
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix("range="))
        .unwrap_or("24h");
    let Some(range) = crate::metrics::Range::parse(range) else {
        return bad("range non valido: usa 1h, 24h o 7d");
    };
    let s = &admin.state;
    let mut out = s.metrics.snapshot(range);
    // aggiunge a ogni instradamento l'indirizzo pubblico, se la regola è ancora attiva
    let snap = s.snapshot.load();
    let matches: std::collections::HashMap<&str, String> = snap
        .routes_by_host
        .values()
        .flatten()
        .map(|r| (r.id.as_str(), format!("{}{}", r.host, r.path_prefix)))
        .collect();
    for list in ["routes", "top_files"] {
        if let Some(items) = out[list].as_array_mut() {
            for it in items {
                let id = it["id"]
                    .as_str()
                    .or(it["route"].as_str())
                    .unwrap_or("")
                    .to_owned();
                it["match"] = matches.get(id.as_str()).map_or(Value::Null, |m| json!(m));
            }
        }
    }
    out["cache"] = json!(s.cache.stats());
    out["config_version"] = json!(snap.version);
    json(StatusCode::OK, out)
}

fn panel_state(admin: &Admin, pr: &Principal) -> Response<Body> {
    let state = &admin.state;
    let panel = match panel::load(&admin.state_dir) {
        Ok(p) => p,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let mut v = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "config_version": state.snapshot.load().version,
        "listen_port": admin.listen_port,
        "http_port": panel.settings.http(),
        "https_port": panel.settings.https(),
        "cache": state.cache.stats(),
        "live_routes": routes_json(state),
        "hand_managed": hand_managed(admin),
        "recheck_minutes": (admin.recheck_pending.as_secs() / 60).max(1),
        "recheck_verified_minutes": (admin.recheck_verified.as_secs() / 60).max(1),
        "panel": panel,
    });
    // chi non può leggere una risorsa non ne vede nemmeno l'elenco
    for (scope, path) in [
        ("domains:read", "domains"),
        ("buckets:read", "buckets"),
        ("routes:read", "rules"),
    ] {
        if !pr.can(scope) {
            v["panel"][path] = json!([]);
        }
    }
    if !pr.can("routes:read") {
        v["live_routes"] = json!([]);
    }
    if pr.can("notifications:manage") {
        v["notify_failing"] = json!(admin.notifier.failing_channels());
    }
    json(StatusCode::OK, v)
}

/// Config scritta a mano e mai passata dal pannello: non la sovrascriviamo.
fn hand_managed(admin: &Admin) -> bool {
    !panel::panel_path(&admin.state_dir).exists() && admin.config_path.exists()
}

// ---------------------------------------------------------------------------
// Verifica dello storage
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct StorageInput {
    endpoint: String,
    #[serde(default = "default_region")]
    region: String,
    #[serde(default = "default_addressing")]
    addressing: Addressing,
    #[serde(default)]
    allow_private_endpoint: bool,
    access_key: String,
    secret_key: String,
}

fn default_region() -> String {
    "us-east-1".into()
}
fn default_addressing() -> Addressing {
    Addressing::Path
}

#[derive(Deserialize)]
struct TestReq {
    #[serde(flatten)]
    storage: StorageInput,
    bucket: String,
    #[serde(default)]
    prefix: String,
    file: String,
}

fn outcome(kind: &str, ok: bool, message: impl Into<String>) -> Value {
    json!({ "outcome": kind, "ok": ok, "message": message.into() })
}

fn build_storage(id: &str, i: &StorageInput) -> Result<config::Storage, String> {
    let endpoint = config::check_endpoint(i.endpoint.trim(), i.allow_private_endpoint)?;
    if i.region.trim().is_empty() {
        return Err("regione vuota".into());
    }
    if i.access_key.is_empty() || i.secret_key.is_empty() {
        return Err("inserisci access key e secret key".into());
    }
    let client =
        s3::build_client(i.allow_private_endpoint).map_err(|e| format!("client HTTP: {e}"))?;
    Ok(config::Storage {
        id: id.to_owned(),
        endpoint,
        region: i.region.trim().to_owned(),
        addressing: i.addressing,
        access_key: i.access_key.clone(),
        secret_key: i.secret_key.clone(),
        client,
    })
}

fn check_bucket(bucket: &str) -> Result<(), String> {
    if bucket.is_empty() || bucket.contains('/') || bucket.len() > 63 {
        return Err(format!("bucket '{bucket}' non valido"));
    }
    Ok(())
}

/// Un GET firmato di un byte sul file di prova. Molti bucket privati non
/// permettono l'elenco, quindi non usiamo ListObjects: con credenziali sbagliate
/// lo storage risponde con un errore preciso, con un file mancante no.
async fn storage_test(req: &TestReq) -> Value {
    let st = match build_storage("test", &req.storage) {
        Ok(s) => s,
        Err(e) => return outcome("invalid", false, e),
    };
    if let Err(e) = check_bucket(req.bucket.trim()) {
        return outcome("invalid", false, e);
    }
    let prefix = match config::normalize_bucket_prefix(&req.prefix) {
        Ok(p) => p,
        Err(e) => return outcome("invalid", false, format!("cartella: {e}")),
    };
    let file = req.file.trim().trim_start_matches('/');
    if file.is_empty() || file.split('/').any(|s| s == "." || s == "..") {
        return outcome("invalid", false, "indica il percorso di un file di prova");
    }
    let key = format!("{prefix}{file}");
    let probe = [("range".to_string(), "bytes=0-0".to_string())];
    match s3::fetch(&st, req.bucket.trim(), &key, &Method::GET, &probe).await {
        s3::Fetch::Ok(resp) => {
            let size = resp
                .headers()
                .get(header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit('/').next())
                .map(str::to_owned)
                .or_else(|| {
                    resp.headers()
                        .get(header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            let ctype = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            json!({
                "outcome": "found", "ok": true,
                "message": "Connessione riuscita: il file di prova è leggibile.",
                "key": key, "size": size, "content_type": ctype,
            })
        }
        s3::Fetch::NotModified => outcome("found", true, "Connessione riuscita."),
        s3::Fetch::NotFound => outcome(
            "not_found",
            true,
            "Lo storage risponde, ma il file non risulta accessibile: controlla bucket, \
             cartella e nome del file, oppure i permessi di lettura.",
        ),
        s3::Fetch::Misconfigured(m) => outcome(
            "auth",
            false,
            format!("Lo storage ha rifiutato le credenziali o la richiesta ({m})."),
        ),
        s3::Fetch::Upstream(m) => outcome(
            "unreachable",
            false,
            format!("Storage non raggiungibile: {m}"),
        ),
    }
}

async fn test_storage(req: TestReq) -> Response<Body> {
    json(StatusCode::OK, storage_test(&req).await)
}

// ---------------------------------------------------------------------------
// Modifiche allo stato del pannello
// ---------------------------------------------------------------------------

pub(crate) fn bad(m: impl Into<String>) -> Response<Body> {
    error(StatusCode::UNPROCESSABLE_ENTITY, m)
}

fn slug(s: &str, fallback: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    let out: String = out.chars().take(48).collect();
    if out.is_empty() {
        fallback.to_owned()
    } else {
        out
    }
}

fn unique_id(base: String, taken: &dyn Fn(&str) -> bool) -> String {
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap()
}

/// Scrittura atomica; per i segreti il file è leggibile solo dal proprietario.
pub(crate) fn write_atomic(path: &Path, data: &[u8], private: bool) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let _ = private;
    let mut f = opts.open(&tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
}

fn load_panel(admin: &Admin) -> Result<Panel, Response<Body>> {
    panel::load(&admin.state_dir).map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, e))
}

fn save_panel(admin: &Admin, p: &Panel) -> Result<(), Response<Body>> {
    panel::save(&admin.state_dir, p).map_err(|e| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("salvataggio: {e}"),
        )
    })
}

fn guard_hand_managed(admin: &Admin) -> Result<(), Response<Body>> {
    if hand_managed(admin) {
        return Err(error(
            StatusCode::CONFLICT,
            "la configurazione è gestita a mano (config.yaml senza stato del pannello): \
             il pannello non la sovrascrive",
        ));
    }
    Ok(())
}

/// Rigenera la configurazione dallo stato del pannello, la valida e la applica
/// subito. Le credenziali dei bucket devono essere già su disco.
fn apply_config(admin: &Admin, p: &Panel) -> Result<u64, String> {
    let file_version = std::fs::read(&admin.config_path)
        .ok()
        .and_then(|raw| serde_yaml::from_slice::<Value>(&raw).ok())
        .and_then(|v| v["version"].as_u64())
        .unwrap_or(0);
    let version = file_version.max(admin.state.snapshot.load().version) + 1;
    let secrets = panel::secrets_dir(&admin.state_dir).map_err(|e| format!("state_dir: {e}"))?;
    let yaml = serde_yaml::to_string(&panel::generate_config(p, version, &secrets))
        .map_err(|e| format!("serializzazione: {e}"))?;
    let snapshot = config::parse(yaml.as_bytes()).map_err(|e| e.to_string())?;
    write_atomic(&admin.config_path, yaml.as_bytes(), false)
        .map_err(|e| format!("scrittura config: {e}"))?;
    crate::save_last_good(&admin.state_dir.join("last-good.yaml"), yaml.as_bytes());
    tracing::info!(
        from = version - 1,
        to = version,
        routes = snapshot.route_count(),
        "configurazione applicata dal pannello"
    );
    admin.state.snapshot.store(Arc::new(snapshot));
    Ok(version)
}

fn secret_path(admin: &Admin, id: &str) -> Result<PathBuf, Response<Body>> {
    panel::secrets_dir(&admin.state_dir)
        .map(|d| d.join(format!("{id}.json")))
        .map_err(|e| error(StatusCode::INTERNAL_SERVER_ERROR, format!("state_dir: {e}")))
}

// --- impostazioni ---------------------------------------------------------

#[derive(Deserialize)]
struct SettingsReq {
    /// vuoto = porta standard
    #[serde(default)]
    http_port: Option<u16>,
    #[serde(default)]
    https_port: Option<u16>,
}

async fn save_settings(admin: &Admin, req: SettingsReq) -> Response<Body> {
    if req.http_port == Some(0) || req.https_port == Some(0) {
        return bad("porta non valida");
    }
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    p.settings.http_port = req.http_port.filter(|&n| n != panel::DEFAULT_HTTP_PORT);
    p.settings.https_port = req.https_port.filter(|&n| n != panel::DEFAULT_HTTPS_PORT);
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log(
        "settings.update",
        &format!("http {} · https {}", p.settings.http(), p.settings.https()),
    );
    json(
        StatusCode::OK,
        json!({ "http_port": p.settings.http(), "https_port": p.settings.https() }),
    )
}

// --- notifiche --------------------------------------------------------------

#[derive(Deserialize)]
struct NotifyReq {
    config: crate::notify::Config,
    /// vuoto o assente = mantieni quella salvata
    #[serde(default)]
    smtp_password: Option<String>,
    #[serde(default)]
    telegram_token: Option<String>,
}

#[derive(Deserialize)]
struct NotifyTestReq {
    channel: String,
}

fn notifications_get(admin: &Admin) -> Response<Body> {
    let cfg = match crate::notify::load_config(&admin.state_dir) {
        Ok(c) => c,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let secrets = crate::notify::load_secrets(&admin.state_dir);
    json(
        StatusCode::OK,
        json!({
            "config": cfg,
            "has_smtp_password": !secrets.smtp_password.is_empty(),
            "has_telegram_token": !secrets.telegram_token.is_empty(),
            "log": admin.notifier.log(),
        }),
    )
}

async fn notifications_put(admin: &Admin, req: NotifyReq) -> Response<Body> {
    if let Err(e) = req.config.validate() {
        return bad(e);
    }
    let _g = admin.write_lock.lock().await;
    let mut secrets = crate::notify::load_secrets(&admin.state_dir);
    if let Some(p) = req.smtp_password.filter(|p| !p.is_empty()) {
        secrets.smtp_password = p;
    }
    if let Some(t) = req
        .telegram_token
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        secrets.telegram_token = t;
    }
    if req.config.telegram.enabled && secrets.telegram_token.is_empty() {
        return bad("indica il token del bot Telegram");
    }
    if let Err(e) = crate::notify::save_secrets(&admin.state_dir, &secrets)
        .and_then(|()| crate::notify::save_config(&admin.state_dir, &req.config))
    {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("salvataggio: {e}"),
        );
    }
    // mai i segreti nel registro attività
    audit::log(
        "notifications.update",
        &format!(
            "email {} · telegram {}",
            if req.config.email.enabled {
                "on"
            } else {
                "off"
            },
            if req.config.telegram.enabled {
                "on"
            } else {
                "off"
            }
        ),
    );
    notifications_get(admin)
}

async fn notifications_test(admin: &Admin, req: NotifyTestReq) -> Response<Body> {
    let res = admin.notifier.test(&req.channel).await;
    audit::log(
        "notifications.test",
        &format!(
            "{} · {}",
            req.channel,
            if res.is_ok() { "ok" } else { "errore" }
        ),
    );
    match res {
        Ok(()) => json(
            StatusCode::OK,
            json!({ "ok": true, "log": admin.notifier.log() }),
        ),
        Err(e) => json(
            StatusCode::OK,
            json!({ "ok": false, "error": e, "log": admin.notifier.log() }),
        ),
    }
}

// --- domini ---------------------------------------------------------------

#[derive(Deserialize)]
struct HostReq {
    host: String,
}

async fn add_domain(admin: &Admin, req: HostReq) -> Response<Body> {
    let host = match config::normalize_host(&req.host) {
        Ok(h) if h.parse::<IpAddr>().is_err() && h.contains(|c: char| c.is_ascii_alphabetic()) => h,
        Ok(_) => return bad("inserisci un nome di dominio, non un indirizzo IP"),
        Err(e) => return bad(format!("dominio: {e}")),
    };
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if p.domains.iter().any(|d| d.host == host) {
        return error(StatusCode::CONFLICT, format!("{host} è già censito"));
    }
    let mut d = Domain {
        host: host.clone(),
        verified: false,
        checked_at: None,
        records: vec![],
        message: String::new(),
        stages: vec![],
        ever_verified: false,
        since: None,
    };
    d.apply(dns::check_domain(&host, p.settings.http(), &admin.state.node_id).await);
    p.domains.push(d.clone());
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log("domain.add", &host);
    json(StatusCode::OK, json!(d))
}

/// Prova un dominio senza censirlo: la usa la finestra "Nuovo dominio".
async fn test_domain(admin: &Admin, req: HostReq) -> Response<Body> {
    let host = match config::normalize_host(&req.host) {
        Ok(h) if h.parse::<IpAddr>().is_err() && h.contains(|c: char| c.is_ascii_alphabetic()) => h,
        Ok(_) => return bad("inserisci un nome di dominio, non un indirizzo IP"),
        Err(e) => return bad(format!("dominio: {e}")),
    };
    let p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let r = dns::check_domain(&host, p.settings.http(), &admin.state.node_id).await;
    json(StatusCode::OK, json!({ "host": host, "result": r }))
}

async fn check_domain(admin: &Admin, req: HostReq) -> Response<Body> {
    let host = req.host.trim().to_ascii_lowercase();
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let port = p.settings.http();
    let Some(d) = p.domains.iter_mut().find(|d| d.host == host) else {
        return error(StatusCode::NOT_FOUND, "dominio non censito");
    };
    d.apply(dns::check_domain(&host, port, &admin.state.node_id).await);
    let out = json!(d);
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    json(StatusCode::OK, out)
}

async fn delete_domain(admin: &Admin, host: &str) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if p.rules.iter().any(|r| r.domain == host) {
        return error(
            StatusCode::CONFLICT,
            "elimina prima gli instradamenti di questo dominio",
        );
    }
    let before = p.domains.len();
    p.domains.retain(|d| d.host != host);
    if p.domains.len() == before {
        return error(StatusCode::NOT_FOUND, "dominio non censito");
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log("domain.delete", host);
    json(StatusCode::OK, json!({"ok": true}))
}

/// Un controllo è dovuto se non è mai stato fatto o se è più vecchio di `every`.
fn is_due(checked_at: Option<&str>, every: Duration) -> bool {
    checked_at
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .is_none_or(|t| {
            chrono::Utc::now()
                .signed_duration_since(t)
                .to_std()
                .unwrap_or_default()
                >= every
        })
}

/// Richiesta di prova per un bucket già censito, con le credenziali salvate.
fn bucket_test_req(admin: &Admin, b: &Bucket) -> Result<TestReq, String> {
    let secret = panel::secrets_dir(&admin.state_dir)
        .map(|d| d.join(format!("{}.json", b.id)))
        .map_err(|e| format!("state_dir: {e}"))?;
    let creds: Value = std::fs::read(&secret)
        .ok()
        .and_then(|r| serde_json::from_slice(&r).ok())
        .ok_or("credenziali non leggibili")?;
    Ok(TestReq {
        storage: StorageInput {
            endpoint: b.endpoint.clone(),
            region: b.region.clone(),
            addressing: b.addressing,
            allow_private_endpoint: b.allow_private_endpoint,
            access_key: creds["access_key"].as_str().unwrap_or_default().to_owned(),
            secret_key: creds["secret_key"].as_str().unwrap_or_default().to_owned(),
        },
        bucket: b.bucket.clone(),
        prefix: String::new(),
        file: b.test_file.clone(),
    })
}

/// Ricontrollo periodico di domini e bucket. Quelli in attesa spesso diventano
/// validi solo dopo ore (propagazione DNS); quelli verificati vanno tenuti
/// d'occhio per accorgersi subito se un record smette di funzionare.
pub async fn recheck_loop(admin: Arc<Admin>) {
    let mut tick = tokio::time::interval(
        (admin.recheck_pending / 2).clamp(Duration::from_secs(1), Duration::from_secs(60)),
    );
    loop {
        tick.tick().await;
        let due = |d: &Domain| {
            let every = if d.verified {
                admin.recheck_verified
            } else {
                admin.recheck_pending
            };
            is_due(d.checked_at.as_deref(), every)
        };
        let (hosts, port) = match panel::load(&admin.state_dir) {
            Ok(p) => (
                p.domains
                    .iter()
                    .filter(|d| due(d))
                    .map(|d| (d.host.clone(), d.verified))
                    .collect::<Vec<_>>(),
                p.settings.http(),
            ),
            Err(_) => continue,
        };
        for (host, was_verified) in hosts {
            // il controllo (lento) avviene fuori dal lock di scrittura
            let mut result = dns::check_domain(&host, port, &admin.state.node_id).await;
            if !result.ok && was_verified {
                // un timeout isolato non deve far scattare l'errore: secondo tentativo
                tokio::time::sleep(admin.recheck_pending.min(Duration::from_secs(10))).await;
                result = dns::check_domain(&host, port, &admin.state.node_id).await;
            }
            let _g = admin.write_lock.lock().await;
            let Ok(mut p) = panel::load(&admin.state_dir) else {
                continue;
            };
            if let Some(d) = p.domains.iter_mut().find(|d| d.host == host) {
                if d.verified != result.ok {
                    if result.ok {
                        tracing::info!(host = %host, "dominio ora verificato");
                    } else if was_verified {
                        tracing::warn!(host = %host, motivo = %result.message, "errore DNS: il dominio non risulta più valido");
                    }
                }
                d.apply(result);
                let _ = panel::save(&admin.state_dir, &p);
            }
        }
        recheck_buckets(&admin).await;
    }
}

/// Ricontrollo dei bucket con le credenziali salvate: se lo storage smette di
/// rispondere (o rifiuta le chiavi) lo stato cambia e compare tra gli avvisi.
async fn recheck_buckets(admin: &Admin) {
    let due: Vec<(String, bool)> = match panel::load(&admin.state_dir) {
        Ok(p) => p
            .buckets
            .iter()
            .filter(|b| {
                let (ok, at) = b
                    .check
                    .as_ref()
                    .map_or((false, None), |c| (c.ok, Some(c.checked_at.as_str())));
                let every = if ok {
                    admin.recheck_verified
                } else {
                    admin.recheck_pending
                };
                is_due(at, every)
            })
            .map(|b| (b.id.clone(), b.check.as_ref().is_some_and(|c| c.ok)))
            .collect(),
        Err(_) => return,
    };
    for (id, was_ok) in due {
        let Ok(p) = panel::load(&admin.state_dir) else {
            return;
        };
        let Some(bucket) = p.buckets.iter().find(|b| b.id == id).cloned() else {
            continue;
        };
        let req = match bucket_test_req(admin, &bucket) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(bucket = %id, error = %e, "impossibile ricontrollare il bucket");
                continue;
            }
        };
        // il controllo (lento) avviene fuori dal lock di scrittura
        let mut check = bucket_check(&storage_test(&req).await);
        if !check.ok && was_ok {
            // un timeout isolato non deve far scattare l'errore: secondo tentativo
            tokio::time::sleep(admin.recheck_pending.min(Duration::from_secs(10))).await;
            check = bucket_check(&storage_test(&req).await);
        }
        let _g = admin.write_lock.lock().await;
        let Ok(mut p) = panel::load(&admin.state_dir) else {
            continue;
        };
        if let Some(b) = p.buckets.iter_mut().find(|b| b.id == id) {
            if was_ok && !check.ok {
                tracing::warn!(bucket = %id, motivo = %check.message, "il bucket non risulta più raggiungibile");
            } else if !was_ok && check.ok {
                tracing::info!(bucket = %id, "bucket di nuovo raggiungibile");
            }
            b.check = Some(check);
            let _ = panel::save(&admin.state_dir, &p);
        }
    }
}

// --- bucket ---------------------------------------------------------------

#[derive(Deserialize)]
struct BucketReq {
    name: String,
    #[serde(flatten)]
    storage: StorageInput,
    bucket: String,
    /// file letto per verificare la connessione (anche nei controlli successivi)
    file: String,
}

#[derive(Deserialize)]
struct IdReq {
    id: String,
}

fn bucket_check(v: &Value) -> BucketCheck {
    BucketCheck {
        outcome: v["outcome"].as_str().unwrap_or("invalid").to_owned(),
        ok: v["ok"].as_bool().unwrap_or(false),
        message: v["message"].as_str().unwrap_or_default().to_owned(),
        checked_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    }
}

/// Ricontrolla un bucket già censito con le credenziali salvate.
async fn check_bucket_saved(admin: &Admin, req: IdReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Some(b) = p.buckets.iter_mut().find(|b| b.id == req.id) else {
        return error(StatusCode::NOT_FOUND, "bucket non trovato");
    };
    let test = match bucket_test_req(admin, b) {
        Ok(t) => t,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let check = bucket_check(&storage_test(&test).await);
    b.check = Some(check.clone());
    let out = json!(check);
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    json(StatusCode::OK, out)
}

async fn add_bucket(admin: &Admin, req: BucketReq) -> Response<Body> {
    let name = req.name.trim();
    if name.is_empty() {
        return bad("dai un nome al bucket");
    }
    if let Err(e) = build_storage("x", &req.storage) {
        return bad(e);
    }
    if let Err(e) = check_bucket(req.bucket.trim()) {
        return bad(e);
    }
    let _g = admin.write_lock.lock().await;
    if let Err(r) = guard_hand_managed(admin) {
        return r;
    }
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let id = unique_id(slug(name, "bucket"), &|id| {
        p.buckets.iter().any(|b| b.id == id)
    });
    let secret = match secret_path(admin, &id) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let body = json!({"access_key": req.storage.access_key, "secret_key": req.storage.secret_key});
    if let Err(e) = write_atomic(&secret, body.to_string().as_bytes(), true) {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("salvataggio credenziali: {e}"),
        );
    }
    let test = TestReq {
        storage: StorageInput {
            endpoint: req.storage.endpoint.clone(),
            region: req.storage.region.clone(),
            addressing: req.storage.addressing,
            allow_private_endpoint: req.storage.allow_private_endpoint,
            access_key: req.storage.access_key.clone(),
            secret_key: req.storage.secret_key.clone(),
        },
        bucket: req.bucket.trim().to_owned(),
        prefix: String::new(),
        file: req.file.clone(),
    };
    let check = bucket_check(&storage_test(&test).await);
    let bucket = Bucket {
        id: id.clone(),
        name: name.to_owned(),
        endpoint: req.storage.endpoint.trim().to_owned(),
        region: req.storage.region.trim().to_owned(),
        addressing: req.storage.addressing,
        allow_private_endpoint: req.storage.allow_private_endpoint,
        bucket: req.bucket.trim().to_owned(),
        test_file: req.file.trim().trim_start_matches('/').to_owned(),
        check: Some(check),
    };
    p.buckets.push(bucket.clone());
    if let Err(e) = apply_config(admin, &p) {
        let _ = std::fs::remove_file(&secret);
        return bad(e);
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log("bucket.add", &bucket.name);
    json(StatusCode::OK, json!(bucket))
}

async fn delete_bucket(admin: &Admin, id: &str) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if let Err(r) = guard_hand_managed(admin) {
        return r;
    }
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if p.rules.iter().any(|r| r.bucket_id == id) {
        return error(
            StatusCode::CONFLICT,
            "elimina prima gli instradamenti che usano questo bucket",
        );
    }
    let before = p.buckets.len();
    p.buckets.retain(|b| b.id != id);
    if p.buckets.len() == before {
        return error(StatusCode::NOT_FOUND, "bucket non trovato");
    }
    if let Err(e) = apply_config(admin, &p) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e);
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    if let Ok(s) = secret_path(admin, id) {
        let _ = std::fs::remove_file(s);
    }
    audit::log("bucket.delete", id);
    json(StatusCode::OK, json!({"ok": true}))
}

// --- instradamenti --------------------------------------------------------

#[derive(Deserialize)]
struct RuleReq {
    domain: String,
    #[serde(default = "default_path_prefix")]
    path_prefix: String,
    bucket_id: String,
    #[serde(default)]
    folder: String,
}

fn default_path_prefix() -> String {
    "/".into()
}

async fn add_rule(admin: &Admin, req: RuleReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if let Err(r) = guard_hand_managed(admin) {
        return r;
    }
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let domain = req.domain.trim().to_ascii_lowercase();
    match p.domains.iter().find(|d| d.host == domain) {
        None => return bad("il dominio non è censito"),
        Some(d) if !d.verified => {
            return bad(
                "il dominio non è ancora verificato: attendi che i record DNS siano propagati",
            )
        }
        Some(_) => {}
    }
    if !p.buckets.iter().any(|b| b.id == req.bucket_id) {
        return bad("il bucket non esiste");
    }
    let prefix = match config::normalize_route_prefix(req.path_prefix.trim()) {
        Ok(x) => x,
        Err(e) => return bad(format!("prefisso: {e}")),
    };
    let folder = match config::normalize_bucket_prefix(&req.folder) {
        Ok(x) => x,
        Err(e) => return bad(format!("cartella: {e}")),
    };
    let id = unique_id(slug(&format!("{domain}{prefix}"), "regola"), &|id| {
        p.rules.iter().any(|r| r.id == id)
    });
    let rule = Rule {
        id,
        domain,
        path_prefix: prefix,
        bucket_id: req.bucket_id,
        folder,
    };
    p.rules.push(rule.clone());
    if let Err(e) = apply_config(admin, &p) {
        return bad(e);
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log("rule.add", &format!("{}{}", rule.domain, rule.path_prefix));
    json(StatusCode::OK, json!(rule))
}

async fn delete_rule(admin: &Admin, id: &str) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if let Err(r) = guard_hand_managed(admin) {
        return r;
    }
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let before = p.rules.len();
    p.rules.retain(|r| r.id != id);
    if p.rules.len() == before {
        return error(StatusCode::NOT_FOUND, "instradamento non trovato");
    }
    if let Err(e) = apply_config(admin, &p) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e);
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log("rule.delete", id);
    json(StatusCode::OK, json!({"ok": true}))
}

// ---------------------------------------------------------------------------
// Prova dell'URL pubblico
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ProbeReq {
    host: String,
    path: String,
}

/// Fa una richiesta al proprio listener pubblico con l'header Host scelto,
/// come farebbe un visitatore, e riporta stato e `X-Cache`.
async fn probe(admin: &Admin, req: ProbeReq) -> Response<Body> {
    let host = match config::normalize_host(&req.host) {
        Ok(h) => h,
        Err(e) => return error(StatusCode::UNPROCESSABLE_ENTITY, format!("host: {e}")),
    };
    if !req.path.starts_with('/') || req.path.contains(['?', '#', ' ']) {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "il percorso deve iniziare con / e non avere query",
        );
    }
    let url = format!("http://127.0.0.1:{}{}", admin.listen_port, req.path);
    let client = match s3::build_client(true) {
        Ok(c) => c,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let t0 = Instant::now();
    let resp = match client.get(&url).header(header::HOST, &host).send().await {
        Ok(r) => r,
        Err(e) => {
            return error(
                StatusCode::BAD_GATEWAY,
                format!("richiesta al gateway: {e}"),
            )
        }
    };
    let h = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let status = resp.status().as_u16();
    let (x_cache, ctype, len) = (h("x-cache"), h("content-type"), h("content-length"));
    let small_text = ctype.as_deref().is_some_and(|c| c.starts_with("text/"))
        && len
            .as_deref()
            .and_then(|l| l.parse::<u64>().ok())
            .is_some_and(|l| l <= 2048);
    let preview = if small_text {
        resp.text().await.ok()
    } else {
        None // il resto del corpo viene scartato senza scaricarlo
    };
    json(
        StatusCode::OK,
        json!({
            "status": status,
            "x_cache": x_cache,
            "content_type": ctype,
            "content_length": len,
            "elapsed_ms": t0.elapsed().as_millis() as u64,
            "preview": preview,
        }),
    )
}

// ---------------------------------------------------------------------------
// File statici del pannello
// ---------------------------------------------------------------------------

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// La guida offline: file statici sotto `/docs/`, con `index.html` per le cartelle.
fn serve_docs(admin: &Admin, path: &str) -> Response<Body> {
    let Some(dir) = admin.docs_dir.as_ref().filter(|d| d.is_dir()) else {
        return error(
            StatusCode::NOT_FOUND,
            "guida non installata: imposta OTR_DOCS_DIR",
        );
    };
    let rel = path
        .strip_prefix("/docs")
        .unwrap_or("")
        .trim_start_matches('/');
    if rel.contains('\0') || rel.split('/').any(|s| s == ".." || s == ".") {
        return error(StatusCode::NOT_FOUND, "not found");
    }
    let mut file = dir.join(rel);
    if file.is_dir() {
        file = file.join("index.html");
    } else if !file.is_file() && !rel.ends_with(".html") {
        file = dir.join(format!("{rel}.html"));
    }
    match std::fs::read(&file) {
        Ok(data) => {
            let mut r = Response::new(body::full(data));
            r.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime(&file)));
            r.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static(if rel.starts_with("assets/") {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-cache"
                }),
            );
            r
        }
        Err(_) => error(StatusCode::NOT_FOUND, "not found"),
    }
}

fn serve_static(admin: &Admin, path: &str) -> Response<Body> {
    let rel = path.trim_start_matches('/');
    let safe = !rel.contains('\0') && rel.split('/').all(|s| s != ".." && s != ".");
    let mut file = admin
        .ui_dir
        .join(if rel.is_empty() { "index.html" } else { rel });
    if !safe {
        return error(StatusCode::NOT_FOUND, "not found");
    }
    if !file.is_file() {
        // fallback SPA solo per percorsi "di pagina", non per asset mancanti
        if rel.rsplit('/').next().is_some_and(|l| l.contains('.')) {
            return error(StatusCode::NOT_FOUND, "not found");
        }
        file = admin.ui_dir.join("index.html");
    }
    match std::fs::read(&file) {
        Ok(data) => {
            let mut r = Response::new(body::full(data));
            r.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime(&file)));
            let cache = if rel.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            r.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
            r
        }
        Err(_) => error(
            StatusCode::NOT_FOUND,
            "pannello non trovato: esegui `npm run build` in web/ o imposta OTR_UI_DIR",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ogni route dichiarata nel codice deve comparire nella tabella dei permessi:
    /// un endpoint dimenticato resterebbe senza controllo degli scope.
    #[test]
    fn every_api_route_has_a_permission() {
        // questi si gestiscono prima dell'autenticazione (login, sessione, setup)
        const OPEN: &[&str] = &[
            "/api/",
            "/api/session",
            "/api/setup",
            "/api/login",
            "/api/login/2fa",
            "/api/logout",
        ];
        let mut checked = 0;
        for full in [include_str!("admin.rs"), include_str!("admin_users.rs")] {
            // solo il codice, senza il modulo dei test (che contiene queste stesse stringhe)
            let src = full.split("#[cfg(test)]").next().unwrap_or(full);
            for line in src
                .lines()
                .filter(|l| l.contains("(&Method::") && l.contains("\"/api/"))
            {
                let method = ["GET", "POST", "PUT", "DELETE"]
                    .into_iter()
                    .find(|m| line.contains(&format!("&Method::{m}")))
                    .unwrap_or_else(|| panic!("metodo non riconosciuto: {line}"));
                for lit in line.split('"').filter(|t| t.starts_with("/api/")) {
                    if OPEN.contains(&lit) {
                        continue;
                    }
                    // un prefisso ("/api/domains/") si prova con un id qualunque
                    let path = if lit.ends_with('/') {
                        format!("{lit}x")
                    } else {
                        lit.to_owned()
                    };
                    let m = Method::from_bytes(method.as_bytes()).unwrap();
                    assert!(
                        access(&m, &path).is_some(),
                        "manca il permesso per {method} {path}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked >= 20, "il test non trova le route ({checked})");
    }

    #[test]
    fn permission_matrix() {
        fn can(role: &str, method: Method, path: &str) -> bool {
            let u = crate::users::User {
                id: "x".into(),
                username: "x".into(),
                password_hash: String::new(),
                role: role.into(),
                scopes: vec![],
                disabled: false,
                must_change_password: false,
                totp: Default::default(),
                recovery_hashes: vec![],
                created_at: String::new(),
                last_login_at: None,
            };
            match access(&method, path) {
                Some(Access::Open) => true,
                Some(Access::Scope(s)) => u.can(s),
                None => false,
            }
        }
        use Method as M;
        // la sola lettura non modifica nulla
        for (m, p) in [
            (M::POST, "/api/domains"),
            (M::DELETE, "/api/domains/a.it"),
            (M::POST, "/api/buckets"),
            (M::POST, "/api/rules"),
            (M::PUT, "/api/settings"),
            (M::GET, "/api/users"),
            (M::GET, "/api/audit"),
        ] {
            assert!(
                !can("viewer", m.clone(), p),
                "viewer non deve poter {m} {p}"
            );
        }
        assert!(can("viewer", M::GET, "/api/metrics") && can("viewer", M::POST, "/api/probe"));
        // l'operatore lavora su domini/bucket/instradamenti, non su impostazioni e utenti
        assert!(
            can("operator", M::POST, "/api/domains") && can("operator", M::DELETE, "/api/rules/r")
        );
        assert!(
            !can("operator", M::PUT, "/api/settings") && !can("operator", M::POST, "/api/users")
        );
        assert!(
            !can("operator", M::PUT, "/api/policy") && !can("operator", M::PUT, "/api/users/1")
        );
        // l'amministratore può tutto ciò che esiste
        assert!(
            can("admin", M::PUT, "/api/settings")
                && can("admin", M::POST, "/api/users/1/reset-2fa")
        );
        // il profilo personale è di tutti, anche della sola lettura
        assert!(
            can("viewer", M::PUT, "/api/me/password")
                && can("viewer", M::POST, "/api/me/2fa/start")
        );
        // cose che non esistono non sono permesse a nessuno
        assert!(!can("admin", M::GET, "/api/segreto"));
    }

    #[test]
    fn limited_sessions_only_reach_their_own_endpoints() {
        use Requirement::*;
        assert!(allowed_while_limited(
            ChangePassword,
            &Method::PUT,
            "/api/me/password"
        ));
        assert!(!allowed_while_limited(
            ChangePassword,
            &Method::GET,
            "/api/panel"
        ));
        assert!(!allowed_while_limited(
            ChangePassword,
            &Method::POST,
            "/api/me/2fa/start"
        ));
        assert!(allowed_while_limited(
            SetupTwoFactor,
            &Method::POST,
            "/api/me/2fa/confirm"
        ));
        assert!(!allowed_while_limited(
            SetupTwoFactor,
            &Method::GET,
            "/api/panel"
        ));
        assert!(!allowed_while_limited(
            SetupTwoFactor,
            &Method::POST,
            "/api/domains"
        ));
        assert!(allowed_while_limited(
            SetupTwoFactor,
            &Method::GET,
            "/api/me"
        ));
    }

    #[test]
    fn slugs_and_unique_ids() {
        assert_eq!(slug("Storage Cliente #1", "x"), "storage-cliente-1");
        assert_eq!(slug("  ", "fallback"), "fallback");
        let taken = |id: &str| id == "a" || id == "a-2";
        assert_eq!(unique_id("a".into(), &taken), "a-3");
        assert_eq!(unique_id("b".into(), &taken), "b");
    }
}
