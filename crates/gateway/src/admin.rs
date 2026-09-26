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
use crate::sign;

const MAX_BODY: usize = 64 * 1024;
/// Il dominio (con certificato) su cui il pannello si serve in HTTPS dal listener pubblico.
#[derive(Default)]
pub struct AdminGate {
    host: std::sync::RwLock<Option<String>>,
}

impl AdminGate {
    pub fn set(&self, host: Option<String>) {
        *self.host.write().unwrap() = host;
    }
    pub fn get(&self) -> Option<String> {
        self.host.read().unwrap().clone()
    }
    /// La richiesta è per il dominio del pannello?
    pub fn is_admin<B>(&self, req: &Request<B>) -> bool {
        let Some(h) = self.get() else { return false };
        crate::routing::host_of(req.uri(), req.headers()).is_some_and(|x| x == h)
    }
}

tokio::task_local! {
    /// la richiesta arriva su una connessione TLS: il cookie di sessione diventa `Secure`
    static SECURE: bool;
}

/// Il pannello servito dal listener pubblico HTTPS.
pub async fn handle_secure(
    admin: Arc<Admin>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Body>, Infallible> {
    SECURE.scope(true, handle(admin, req)).await
}

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
    pub acme: Arc<crate::acme::Acme>,
    pub gate: Arc<AdminGate>,
    pub updater: Arc<crate::update::Updater>,
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
        ("PUT", "/api/settings" | "/api/https" | "/api/admin-host" | "/api/updates") => {
            Scope("settings:write")
        }
        ("POST", "/api/update/check" | "/api/update/apply") => Scope("settings:write"),
        (
            "POST",
            "/api/domains"
            | "/api/domains/check"
            | "/api/domains/test"
            | "/api/domains/redirect"
            | "/api/certs/issue"
            | "/api/certs/upload",
        ) => Scope("domains:write"),
        ("DELETE", p) if p.starts_with("/api/domains/") => Scope("domains:write"),
        ("POST", "/api/buckets" | "/api/buckets/check" | "/api/buckets/test") => {
            Scope("buckets:write")
        }
        ("DELETE", p) if p.starts_with("/api/buckets/") => Scope("buckets:write"),
        ("POST", "/api/rules") => Scope("routes:write"),
        ("DELETE" | "PUT", p) if p.starts_with("/api/rules/") => Scope("routes:write"),
        ("POST", "/api/links") => Scope("routes:write"),
        ("POST", "/api/links/rotate") => Scope("settings:write"),
        ("POST", "/api/probe") => Scope("routes:read"),
        ("POST", "/api/purge" | "/api/warm") => Scope("routes:write"),
        ("POST", "/api/diagnose") => Scope("routes:read"),
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
        (&Method::PUT, "/api/https") => with_body!(HttpsReq, save_https),
        (&Method::PUT, "/api/updates") => with_body!(UpdatesReq, save_updates),
        (&Method::POST, "/api/update/check") => update_check(admin, pr).await,
        (&Method::POST, "/api/update/apply") => update_apply(admin),
        (&Method::PUT, "/api/admin-host") => with_body!(AdminHostReq, set_admin_host),
        (&Method::POST, "/api/certs/issue") => with_body!(HostReq, issue_cert),
        (&Method::POST, "/api/certs/upload") => with_body!(UploadCertReq, upload_cert),
        (&Method::POST, "/api/domains/redirect") => with_body!(RedirectReq, set_redirect),
        (&Method::POST, "/api/domains/check") => with_body!(HostReq, check_domain),
        (&Method::POST, "/api/domains/test") => with_body!(HostReq, test_domain),
        (&Method::POST, "/api/buckets/check") => with_body!(IdReq, check_bucket_saved),
        (&Method::POST, "/api/buckets/test") => match read_json::<TestReq>(req).await {
            Ok(b) => test_storage(b).await,
            Err(r) => r,
        },
        (&Method::POST, "/api/buckets") => with_body!(BucketReq, add_bucket),
        (&Method::POST, "/api/rules") => with_body!(RuleReq, add_rule),
        (&Method::PUT, p) if p.starts_with("/api/rules/") => {
            let id = p["/api/rules/".len()..].to_owned();
            match read_json::<RuleSignedReq>(req).await {
                Ok(b) => set_rule_signed(admin, &id, b).await,
                Err(r) => r,
            }
        }
        (&Method::POST, "/api/links") => with_body!(LinkReq, create_link),
        (&Method::POST, "/api/links/rotate") => rotate_links(admin).await,
        (&Method::POST, "/api/probe") => with_body!(ProbeReq, probe),
        (&Method::POST, "/api/diagnose") => with_body!(DiagnoseReq, diagnose),
        (&Method::POST, "/api/purge") => with_body!(PurgeReq, purge),
        (&Method::POST, "/api/warm") => with_body!(WarmReq, warm),
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

/// `; Secure` se la richiesta arriva su TLS.
fn secure_attr() -> &'static str {
    if SECURE.try_with(|s| *s).unwrap_or(false) {
        "; Secure"
    } else {
        ""
    }
}

fn session_cookie(token: &str) -> String {
    format!(
        "{}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}{}",
        auth::COOKIE,
        auth::SESSION_TTL.as_secs(),
        secure_attr()
    )
}

fn clear_cookie() -> String {
    format!(
        "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0{}",
        auth::COOKIE,
        secure_attr()
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
            "version": crate::update::CURRENT,
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

/// Stato dei certificati di ogni dominio, per la tabella Domini e gli avvisi.
fn certs_json(admin: &Admin, p: &Panel) -> Vec<Value> {
    let now = sign::now_secs();
    p.domains
        .iter()
        .filter(|d| crate::acme::certifiable(&d.host))
        .map(|d| {
            let info = crate::tls::read_info(&admin.state_dir, &d.host);
            let status = if admin.acme.is_issuing(&d.host) {
                "issuing"
            } else {
                crate::tls::status(&info, now)
            };
            json!({
                "host": d.host,
                "status": status,
                "not_after": info.not_after,
                "issued_at": info.issued_at,
                "error": info.error,
                "serving": admin.state.tls.store.has(&d.host),
            })
        })
        .collect()
}

fn panel_state(admin: &Admin, pr: &Principal) -> Response<Body> {
    let state = &admin.state;
    let panel = match panel::load(&admin.state_dir) {
        Ok(p) => p,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e),
    };
    let mut v = json!({
        "version": crate::update::CURRENT,
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
        "https_listening": state.tls.is_listening(),
        "update": admin.updater.view(panel.settings.updates.check, panel.settings.updates.prerelease),
        "certs": certs_json(admin, &panel),
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
    admin.state.tls.apply_panel(&p);
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

// --- aggiornamenti -----------------------------------------------------------

#[derive(Deserialize)]
struct UpdatesReq {
    check: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    auto: bool,
    #[serde(default)]
    window_start: Option<u8>,
    #[serde(default)]
    window_end: Option<u8>,
}

async fn save_updates(admin: &Admin, req: UpdatesReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if req.window_start.is_some_and(|h| h > 23) || req.window_end.is_some_and(|h| h > 23) {
        return bad("l'ora deve essere tra 0 e 23");
    }
    let old = p.settings.updates.clone();
    p.settings.updates = panel::UpdateSettings {
        check: req.check,
        prerelease: req.prerelease,
        auto: req.auto,
        window_start: req.window_start.unwrap_or(old.window_start),
        window_end: req.window_end.unwrap_or(old.window_end),
    };
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log(
        "updates.settings",
        &format!(
            "controllo {}{}",
            if req.check { "attivo" } else { "spento" },
            if req.prerelease {
                " · anche pre-release"
            } else {
                ""
            }
        ),
    );
    json(
        StatusCode::OK,
        admin.updater.view(req.check, req.prerelease),
    )
}

/// «Controlla ora»: una richiesta subito, anche se il controllo periodico è spento
/// (è l'utente a chiederlo), salvo `OTR_UPDATE_CHECK=off`.
async fn update_check(admin: &Admin, _pr: &Principal) -> Response<Body> {
    if crate::update::env_disabled() {
        return bad("il controllo è disattivato da OTR_UPDATE_CHECK");
    }
    let p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let res = admin.updater.check().await;
    audit::log(
        "updates.check",
        &res.as_ref().map_or_else(|e| e.clone(), |()| "ok".into()),
    );
    json(
        StatusCode::OK,
        admin
            .updater
            .view(p.settings.updates.check, p.settings.updates.prerelease),
    )
}

/// «Aggiorna ora»: avvia l'aggiornamento in background; il pannello ne segue i passi da `update.apply`.
/// Se riesce, il nodo si riavvia (la connessione cade): il pannello si ricarica da solo.
fn update_apply(admin: &Admin) -> Response<Body> {
    let (ok, why) = admin.updater.self_update_status();
    if !ok {
        return bad(why.unwrap_or_else(|| "aggiornamento automatico non disponibile".into()));
    }
    if admin.updater.progress().running {
        return error(StatusCode::CONFLICT, "un aggiornamento è già in corso");
    }
    if admin.acme.any_issuing() {
        return error(
            StatusCode::CONFLICT,
            "è in corso l'emissione di un certificato: riprova tra poco",
        );
    }
    let prerelease = load_panel(admin).is_ok_and(|p| p.settings.updates.prerelease);
    let u = admin.updater.clone();
    tokio::spawn(async move {
        let Err(e) = u.apply(prerelease).await;
        tracing::warn!(error = %e, "aggiornamento non riuscito");
    });
    json(StatusCode::ACCEPTED, json!({ "started": true }))
}

// --- pannello in HTTPS --------------------------------------------------------

#[derive(Deserialize)]
struct AdminHostReq {
    /// `null` = disattiva
    host: Option<String>,
}

async fn set_admin_host(admin: &Admin, req: AdminHostReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let host = req
        .host
        .map(|h| h.trim().to_ascii_lowercase())
        .filter(|h| !h.is_empty());
    let mut warnings: Vec<&str> = Vec::new();
    if let Some(h) = &host {
        if !p.domains.iter().any(|d| &d.host == h) {
            return error(StatusCode::NOT_FOUND, "dominio non censito");
        }
        if p.rules.iter().any(|r| &r.domain == h) {
            return bad("questo dominio serve già dei file: scegline uno dedicato al pannello");
        }
        if !admin.state.tls.is_listening() {
            return bad("il nodo non è in ascolto per HTTPS (porta 443 non disponibile): il pannello non sarebbe raggiungibile");
        }
        if !admin.state.tls.store.has(h) {
            return bad("il dominio non ha ancora un certificato: ottienilo o caricalo prima, altrimenti il pannello non si aprirebbe");
        }
        if admin.auth.users.policy().require_2fa == "off" {
            warnings.push("la verifica in due passaggi non è obbligatoria: su un pannello raggiungibile da Internet conviene renderla obbligatoria (Utenti → Sicurezza)");
        }
    }
    p.settings.admin_host = host.clone();
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    admin.gate.set(host.clone());
    audit::log(
        "admin.host",
        &host.clone().unwrap_or_else(|| "disattivato".into()),
    );
    let url = host
        .as_ref()
        .map(|h| admin.state.tls.https_location(h, "/"));
    json(
        StatusCode::OK,
        json!({ "host": host, "url": url, "warnings": warnings }),
    )
}

// --- HTTPS ------------------------------------------------------------------

#[derive(Deserialize)]
struct HttpsReq {
    enabled: bool,
    #[serde(default)]
    email: String,
    #[serde(default)]
    staging: bool,
}

async fn save_https(admin: &Admin, req: HttpsReq) -> Response<Body> {
    if !crate::acme::valid_email(&req.email) {
        return bad("indirizzo email non valido");
    }
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    p.settings.acme = panel::AcmeSettings {
        enabled: req.enabled,
        email: req.email.trim().to_owned(),
        staging: req.staging,
    };
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log(
        "https.update",
        &format!(
            "{}{}",
            if req.enabled {
                "certificati automatici"
            } else {
                "disattivati"
            },
            if req.staging {
                " · prova (staging)"
            } else {
                ""
            }
        ),
    );
    if req.enabled {
        admin.acme.kick();
    }
    json(StatusCode::OK, json!(p.settings.acme))
}

/// Richiede subito il certificato di un dominio (senza aspettare il giro periodico).
async fn issue_cert(admin: &Admin, req: HostReq) -> Response<Body> {
    let host = req.host.trim().to_ascii_lowercase();
    let p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if !p.settings.acme.enabled {
        return bad("attiva prima i certificati automatici in Impostazioni → HTTPS");
    }
    match p.domains.iter().find(|d| d.host == host) {
        None => return error(StatusCode::NOT_FOUND, "dominio non censito"),
        Some(d) if !d.verified => return bad("il dominio non è ancora verificato"),
        Some(_) => {}
    }
    if !crate::acme::certifiable(&host) {
        return bad("questo nome non può avere un certificato pubblico (dominio locale o IP)");
    }
    if admin.acme.is_issuing(&host) {
        return error(
            StatusCode::CONFLICT,
            "è già in corso un'emissione per questo dominio",
        );
    }
    let acme = admin.acme.clone();
    let h = host.clone();
    tokio::spawn(async move {
        let _ = acme.issue(&h).await;
    });
    audit::log("cert.issue", &host);
    json(StatusCode::ACCEPTED, json!({ "started": true }))
}

#[derive(Deserialize)]
struct UploadCertReq {
    host: String,
    /// certificato e catena in PEM
    chain: String,
    /// chiave privata in PEM
    key: String,
}

/// Carica un certificato ottenuto altrove (CA aziendale, wildcard…) per un dominio censito.
async fn upload_cert(admin: &Admin, req: UploadCertReq) -> Response<Body> {
    let host = req.host.trim().to_ascii_lowercase();
    let _g = admin.write_lock.lock().await;
    let p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if !p.domains.iter().any(|d| d.host == host) {
        return error(StatusCode::NOT_FOUND, "dominio non censito");
    }
    let meta = match crate::tls::save_cert(
        &admin.state_dir,
        &host,
        &req.chain,
        &req.key,
        sign::now_secs(),
    ) {
        Ok(m) => m,
        Err(e) => return bad(e),
    };
    if let Err(e) = crate::tls::load_host(&admin.state_dir, &host, &admin.state.tls.store) {
        return bad(e);
    }
    admin.state.tls.apply_panel(&p);
    audit::log("cert.upload", &host);
    json(
        StatusCode::OK,
        json!({ "host": host, "not_after": meta.not_after }),
    )
}

#[derive(Deserialize)]
struct RedirectReq {
    host: String,
    enabled: bool,
}

async fn set_redirect(admin: &Admin, req: RedirectReq) -> Response<Body> {
    let host = req.host.trim().to_ascii_lowercase();
    let _g = admin.write_lock.lock().await;
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    if req.enabled && !admin.state.tls.store.has(&host) {
        return bad("il dominio non ha ancora un certificato valido: ottienilo prima, altrimenti il sito diventerebbe irraggiungibile");
    }
    let Some(d) = p.domains.iter_mut().find(|d| d.host == host) else {
        return error(StatusCode::NOT_FOUND, "dominio non censito");
    };
    d.redirect_https = req.enabled;
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    admin.state.tls.apply_panel(&p);
    audit::log(
        "domain.redirect",
        &format!(
            "{host} · {}",
            if req.enabled {
                "HTTP → HTTPS"
            } else {
                "nessun redirect"
            }
        ),
    );
    json(
        StatusCode::OK,
        json!({ "host": host, "redirect_https": req.enabled }),
    )
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
        redirect_https: false,
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
    if p.settings.admin_host.as_deref() == Some(host) {
        return error(
            StatusCode::CONFLICT,
            "questo dominio serve il pannello: disattivalo prima in Impostazioni",
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
    // il certificato non serve più: si toglie dalla memoria e dal disco
    admin.state.tls.store.remove(host);
    admin.state.tls.apply_panel(&p);
    if crate::tls::safe_host(host) {
        let _ = std::fs::remove_dir_all(crate::tls::certs_dir(&admin.state_dir).join(host));
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
    #[serde(default)]
    signed: bool,
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
    if p.settings.admin_host.as_deref() == Some(domain.as_str()) {
        return bad("questo dominio serve il pannello: disattiva prima il pannello in HTTPS");
    }
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
        cache_generation: 1,
        signed: req.signed,
        images: false,
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
// Link firmati
// ---------------------------------------------------------------------------

/// `?exp=…&sig=…` per le richieste che il nodo fa a se stesso (prova, precarica,
/// diagnosi) verso un instradamento che richiede link firmati; vuoto altrimenti.
fn own_query(admin: &Admin, host: &str, path: &str) -> String {
    let snap = admin.state.snapshot.load_full();
    let Some(route) = snap.match_route(host, path) else {
        return String::new();
    };
    if !route.signed {
        return String::new();
    }
    let exp = sign::now_secs() + 120;
    format!(
        "?{}",
        sign::query(&admin.state.signing_key.load(), host, path, exp)
    )
}

/// Opzioni di un instradamento modificabili dopo la creazione (quelle assenti restano come sono).
#[derive(Deserialize)]
struct RuleSignedReq {
    #[serde(default)]
    signed: Option<bool>,
    #[serde(default)]
    images: Option<bool>,
}

async fn set_rule_signed(admin: &Admin, id: &str, req: RuleSignedReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if let Err(r) = guard_hand_managed(admin) {
        return r;
    }
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Some(rule) = p.rules.iter_mut().find(|r| r.id == id) else {
        return error(StatusCode::NOT_FOUND, "instradamento non trovato");
    };
    if req.signed.is_none() && req.images.is_none() {
        return bad("nessuna opzione da modificare");
    }
    if let Some(s) = req.signed {
        rule.signed = s;
    }
    if let Some(i) = req.images {
        rule.images = i;
    }
    // le copie già in cache non devono restare accessibili senza link (né viceversa)
    // e le varianti di immagine di un'impostazione precedente non hanno più senso
    rule.cache_generation += 1;
    let label = format!("{}{}", rule.domain, rule.path_prefix);
    let (signed, images) = (rule.signed, rule.images);
    if let Err(e) = apply_config(admin, &p) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e);
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log(
        "rule.options",
        &format!(
            "{label} · {} · immagini {}",
            if signed { "link firmati" } else { "pubblico" },
            if images { "al volo" } else { "originali" }
        ),
    );
    json(
        StatusCode::OK,
        json!({ "id": id, "signed": signed, "images": images }),
    )
}

#[derive(Deserialize)]
struct LinkReq {
    rule: String,
    /// percorso come lo chiede un visitatore, es. `/foto/barca.jpg`
    path: String,
    /// validità in secondi (da 1 secondo a 365 giorni)
    ttl_secs: u64,
    #[serde(default)]
    https: bool,
}

async fn create_link(admin: &Admin, req: LinkReq) -> Response<Body> {
    if req.ttl_secs == 0 || req.ttl_secs > sign::MAX_TTL_SECS {
        return bad("la validità deve andare da 1 secondo a 365 giorni");
    }
    let path = match visitor_path(&req.path) {
        Ok(p) => p,
        Err(e) => return bad(e),
    };
    let snap = admin.state.snapshot.load_full();
    let Some(route) = snap
        .routes_by_host
        .values()
        .flatten()
        .find(|r| r.id == req.rule)
    else {
        return error(StatusCode::NOT_FOUND, "instradamento non trovato");
    };
    if !route.signed {
        return bad("questo instradamento non richiede link firmati: attivali prima");
    }
    if route.object_key(&path).is_none() {
        return bad("il percorso non appartiene a questo instradamento o non è un file");
    }
    let exp = sign::now_secs() + req.ttl_secs;
    let q = sign::query(&admin.state.signing_key.load(), &route.host, &path, exp);
    let panel = panel::load(&admin.state_dir).unwrap_or_default();
    let (scheme, port) = if req.https {
        ("https", panel.settings.https())
    } else {
        ("http", panel.settings.http())
    };
    let default_port = if req.https { 443 } else { 80 };
    let host_port = if port == default_port {
        route.host.clone()
    } else {
        format!("{}:{port}", route.host)
    };
    // il percorso si ricodifica come lo farebbe un browser: la firma vale sul percorso decodificato
    let enc = percent_encoding::utf8_percent_encode(&path, PATH_ENCODE).to_string();
    let url = format!("{scheme}://{host_port}{enc}?{q}");
    audit::log(
        "link.create",
        &format!("{}{} · {} s", route.host, path, req.ttl_secs),
    );
    json(StatusCode::OK, json!({ "url": url, "expires_at": exp }))
}

const PATH_ENCODE: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%');

async fn rotate_links(admin: &Admin) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    match sign::rotate(&admin.state_dir) {
        Ok(k) => {
            admin.state.signing_key.store(Arc::new(k));
            audit::log("link.rotate", "");
            json(StatusCode::OK, json!({ "ok": true }))
        }
        Err(e) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("salvataggio: {e}"),
        ),
    }
}

// ---------------------------------------------------------------------------
// Svuotamento e precaricamento della cache
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PurgeReq {
    rule: String,
    /// assente = tutto l'instradamento; altrimenti il percorso di un file, come lo chiede un visitatore
    #[serde(default)]
    path: Option<String>,
}

/// Percorso di un file così come lo chiede un visitatore: inizia con `/`, senza query.
fn visitor_path(p: &str) -> Result<String, String> {
    if !p.starts_with('/') || p.contains(['?', '#', ' ']) {
        return Err("il percorso deve iniziare con / e non avere query".into());
    }
    crate::routing::normalize_path(p).map_err(|_| "percorso non valido".to_string())
}

async fn purge(admin: &Admin, req: PurgeReq) -> Response<Body> {
    if let Some(path) = req.path.as_deref().filter(|p| !p.is_empty()) {
        let path = match visitor_path(path) {
            Ok(p) => p,
            Err(e) => return bad(e),
        };
        let snap = admin.state.snapshot.load_full();
        let Some(route) = snap
            .routes_by_host
            .values()
            .flatten()
            .find(|r| r.id == req.rule)
        else {
            return error(StatusCode::NOT_FOUND, "instradamento non trovato");
        };
        let Some(key) = route.object_key(&path) else {
            return bad("il percorso non appartiene a questo instradamento");
        };
        let ck = crate::routing::cache_key(route, &key, "");
        let existed = admin.state.cache.remove(&ck).await;
        audit::log("cache.purge", &format!("{}{}", route.host, path));
        return json(StatusCode::OK, json!({ "removed": u32::from(existed) }));
    }
    let _g = admin.write_lock.lock().await;
    if let Err(r) = guard_hand_managed(admin) {
        return r;
    }
    let mut p = match load_panel(admin) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let Some(rule) = p.rules.iter_mut().find(|r| r.id == req.rule) else {
        return error(StatusCode::NOT_FOUND, "instradamento non trovato");
    };
    rule.cache_generation += 1;
    let label = format!("{}{}", rule.domain, rule.path_prefix);
    if let Err(e) = apply_config(admin, &p) {
        return error(StatusCode::INTERNAL_SERVER_ERROR, e);
    }
    if let Err(r) = save_panel(admin, &p) {
        return r;
    }
    audit::log("cache.purge", &format!("{label} (tutto)"));
    json(StatusCode::OK, json!({ "all": true }))
}

#[derive(Deserialize)]
struct WarmReq {
    rule: String,
    paths: Vec<String>,
}

const WARM_MAX_PATHS: usize = 200;
const WARM_BUDGET: Duration = Duration::from_secs(120);

/// Chiede al proprio listener i percorsi indicati, così la prima visita è già `HIT`.
async fn warm(admin: &Admin, req: WarmReq) -> Response<Body> {
    if req.paths.is_empty() || req.paths.len() > WARM_MAX_PATHS {
        return bad(format!("indica da 1 a {WARM_MAX_PATHS} percorsi"));
    }
    let host = {
        let snap = admin.state.snapshot.load_full();
        match snap
            .routes_by_host
            .values()
            .flatten()
            .find(|r| r.id == req.rule)
        {
            Some(r) => r.host.clone(),
            None => return error(StatusCode::NOT_FOUND, "instradamento non trovato"),
        }
    };
    let client = match s3::build_client(true) {
        Ok(c) => c,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let started = Instant::now();
    let mut results = Vec::new();
    for raw in &req.paths {
        let path = raw.trim();
        if started.elapsed() > WARM_BUDGET {
            results.push(json!({ "path": path, "status": null, "error": "non eseguito: tempo massimo raggiunto" }));
            continue;
        }
        if let Err(e) = visitor_path(path) {
            results.push(json!({ "path": path, "status": null, "error": e }));
            continue;
        }
        let url = format!(
            "http://127.0.0.1:{}{}{}",
            admin.listen_port,
            path,
            own_query(admin, &host, path)
        );
        let t0 = Instant::now();
        let r = match client.get(&url).header(header::HOST, &host).send().await {
            Ok(mut resp) => {
                let status = resp.status().as_u16();
                let x_cache = resp
                    .headers()
                    .get("x-cache")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                let mut bytes = 0u64;
                let mut err = None;
                loop {
                    match resp.chunk().await {
                        Ok(Some(c)) => bytes += c.len() as u64,
                        Ok(None) => break,
                        Err(e) => {
                            err = Some(e.to_string());
                            break;
                        }
                    }
                }
                json!({ "path": path, "status": status, "x_cache": x_cache, "bytes": bytes,
                        "elapsed_ms": t0.elapsed().as_millis() as u64, "error": err })
            }
            Err(e) => {
                json!({ "path": path, "status": null, "error": format!("richiesta al gateway: {e}") })
            }
        };
        results.push(r);
    }
    audit::log(
        "cache.warm",
        &format!("{} · {} file", req.rule, req.paths.len()),
    );
    json(StatusCode::OK, json!({ "results": results }))
}

fn human_duration(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs} secondi"),
        60..=3599 => format!("{} minuti", secs / 60),
        3600..=172_799 => format!("{} ore", secs / 3600),
        _ => format!("{} giorni", secs / 86_400),
    }
}

// ---------------------------------------------------------------------------
// Diagnosi di un URL
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DiagnoseReq {
    url: String,
}

/// Segue il percorso di una richiesta e dice dove si ferma.
async fn diagnose(admin: &Admin, req: DiagnoseReq) -> Response<Body> {
    use crate::diag::{Status, Step};
    let raw = req.url.trim();
    let with_scheme = if raw.contains("://") {
        raw.to_owned()
    } else {
        format!("http://{raw}")
    };
    let parsed =
        match url::Url::parse(&with_scheme) {
            Ok(u) if matches!(u.scheme(), "http" | "https") && u.host_str().is_some() => u,
            _ => return bad(
                "indirizzo non valido: scrivi per esempio https://cdn.example.com/foto/barca.jpg",
            ),
        };
    let host = match config::normalize_host(parsed.host_str().unwrap_or_default()) {
        Ok(h) => h,
        Err(e) => return bad(format!("host: {e}")),
    };
    let raw_path = if parsed.path().is_empty() {
        "/"
    } else {
        parsed.path()
    };
    let mut steps: Vec<Step> = Vec::new();
    let panel = panel::load(&admin.state_dir).unwrap_or_default();
    let http_port = panel.settings.http();

    // 0. indirizzo
    let mut first = Step::new("url", "Indirizzo", Status::Ok, format!("{host}{raw_path}"));
    if parsed.scheme() == "https" {
        let tls = &admin.state.tls;
        first = if tls.store.has(&host) && tls.is_listening() {
            Step::new(
                "url",
                "Indirizzo",
                Status::Ok,
                format!("{host}{raw_path} — HTTPS servito da questo nodo con il suo certificato."),
            )
        } else if tls.store.has(&host) {
            Step::new(
                "url",
                "Indirizzo",
                Status::Warn,
                format!("{host}{raw_path} — il dominio ha un certificato ma il nodo non è in ascolto per HTTPS."),
            )
            .fix("Libera la porta HTTPS (443, o quella di OTR_HTTPS_LISTEN) e riavvia il nodo.")
        } else {
            Step::new(
                "url",
                "Indirizzo",
                Status::Warn,
                format!("{host}{raw_path} — questo nodo non ha un certificato per il dominio: HTTPS dipende da un proxy davanti."),
            )
            .fix("Attiva i certificati automatici (Impostazioni → HTTPS) e ottieni il certificato del dominio, oppure metti un proxy con TLS davanti al nodo (guida «HTTPS»).")
        };
    }
    steps.push(first);

    // 1. DNS e nodo
    let listed = panel.domains.iter().any(|d| d.host == host);
    let dom = dns::check_domain(&host, http_port, &admin.state.node_id).await;
    let mut dom_step = if dom.ok {
        let recs = if dom.records.is_empty() {
            "dominio locale".to_owned()
        } else {
            dom.records.join(", ")
        };
        Step::new(
            "dns",
            "Il dominio arriva a questo nodo",
            Status::Ok,
            format!("Risolve a {recs} e il nodo risponde sulla porta {http_port}."),
        )
    } else {
        let failing = dom.stages.iter().find(|s| s.status == "fail");
        let fix = match failing.map(|s| s.id.as_str()) {
            Some("reach") => "Il dominio risolve ma la richiesta non arriva a questo nodo: controlla firewall, port forwarding, proxy/CDN e la porta HTTP nelle Impostazioni.",
            _ => "Crea o correggi il record A/AAAA/CNAME del dominio e attendi la propagazione (vedi «Domini e DNS»).",
        };
        Step::new(
            "dns",
            "Il dominio arriva a questo nodo",
            Status::Fail,
            dom.message.clone(),
        )
        .fix(fix)
    };
    if dom.ok && !listed && !dns::is_local_name(&host) {
        dom_step = Step::new(
            "dns",
            "Il dominio arriva a questo nodo",
            Status::Warn,
            format!(
                "{} Non è però tra i domini censiti nel pannello.",
                dom_step.detail
            ),
        )
        .fix("Aggiungilo da Domini per tenerlo sotto controllo e riceverne gli avvisi.");
    }
    steps.push(dom_step);

    // 2. instradamento (si valuta comunque: aiuta anche se il DNS non c'è ancora)
    let snap = admin.state.snapshot.load_full();
    let path = match crate::routing::normalize_path(raw_path) {
        Ok(p) => Some(p),
        Err(_) => {
            steps.push(
                Step::new("route", "Instradamento", Status::Fail, "Il percorso non è valido (contiene «..», barre codificate o caratteri di controllo).")
                    .fix("Usa un percorso normale, come /foto/barca.jpg."),
            );
            None
        }
    };
    let route = path
        .as_deref()
        .and_then(|p| snap.match_route(&host, p).cloned());
    let mut object_key = None;
    if let Some(p) = &path {
        match &route {
            None => {
                let known: Vec<String> = snap
                    .routes_by_host
                    .get(&host)
                    .map(|v| v.iter().map(|r| r.path_prefix.clone()).collect())
                    .unwrap_or_default();
                let (detail, fix) = if known.is_empty() {
                    (
                        "Nessun instradamento per questo dominio: il nodo risponde 404.".to_owned(),
                        "Crea un instradamento da Instradamenti, scegliendo questo dominio."
                            .to_owned(),
                    )
                } else {
                    (format!("Il dominio ha instradamenti solo per: {}. Il percorso {p} non corrisponde a nessuno.", known.join(", ")), "Usa un percorso che inizi con uno di quei prefissi, o crea un instradamento per questo.".to_owned())
                };
                steps.push(Step::new("route", "Instradamento", Status::Fail, detail).fix(fix));
            }
            Some(r) => {
                object_key = r.object_key(p);
                match &object_key {
                    Some(k) => steps.push(Step::new(
                        "route",
                        "Instradamento",
                        Status::Ok,
                        format!("Regola «{}» ({}{}): il file cercato è «{}» nel bucket «{}».", r.id, r.host, r.path_prefix, k, r.dest.bucket),
                    )),
                    None => steps.push(
                        Step::new("route", "Instradamento", Status::Fail, "Il percorso è una cartella o è vuoto dopo il prefisso: OtterRoute non elenca le cartelle e risponde 404.")
                            .fix("Indica il nome di un file, per esempio /foto/barca.jpg."),
                    ),
                }
            }
        }
    }
    let routed = route.clone().zip(object_key.clone());

    // 2b. link firmato, se l'instradamento lo richiede
    if let (Some(r), Some(p)) = (&route, &path) {
        if r.signed {
            let label = "Link firmato";
            let key = admin.state.signing_key.load();
            let step = match sign::verify(&key, &r.host, p, parsed.query(), sign::now_secs()) {
                Ok(()) => {
                    let left = parsed
                        .query_pairs()
                        .find(|(k, _)| k == "exp")
                        .and_then(|(_, v)| v.parse::<u64>().ok())
                        .map_or(0, |e| e.saturating_sub(sign::now_secs()));
                    Step::new("signed", label, Status::Ok, format!("Il link è valido e scade tra {}.", human_duration(left)))
                }
                Err(sign::Denied::Missing) => Step::new(
                    "signed",
                    label,
                    Status::Fail,
                    "Questo instradamento richiede un link firmato: senza «exp» e «sig» nell'indirizzo il nodo risponde 403.",
                )
                .fix("Crea un link da Instradamenti → Link firmati e usa l'indirizzo completo."),
                Err(sign::Denied::Expired) => Step::new("signed", label, Status::Fail, "Il link è scaduto.")
                    .fix("Crea un nuovo link con una validità più lunga."),
                Err(_) => Step::new(
                    "signed",
                    label,
                    Status::Fail,
                    "La firma non è valida: il percorso o la scadenza sono stati modificati, oppure la chiave dei link è stata ruotata.",
                )
                .fix("Crea un nuovo link: non si può correggere a mano."),
            };
            steps.push(step);
        }
    }

    // 3. cache
    let cache_label = "Cache";
    match &routed {
        None => steps.push(Step::skipped("cache", cache_label)),
        Some((r, key)) => {
            let ck = crate::routing::cache_key(r, key, "");
            match admin.state.cache.lookup(&ck).await {
                None => steps.push(Step::new(
                    "cache",
                    cache_label,
                    Status::Ok,
                    "Non è in cache: la prima richiesta lo legge dallo storage (MISS) e lo salva.",
                )),
                Some(e) => {
                    let (st, d) = crate::diag::cache_state(
                        e.meta.status,
                        e.age().as_secs(),
                        e.is_fresh(&r.policy),
                        e.is_usable_stale(&r.policy),
                        r.policy.ttl.as_secs(),
                    );
                    let mut s = Step::new("cache", cache_label, st, d);
                    if e.meta.status == 404 {
                        s = s.fix("Se il file ora esiste, attendi la scadenza della cache negativa (60 s) o svuotala da Instradamenti → Cache.");
                    }
                    steps.push(s);
                }
            }
        }
    }

    // 4. storage
    let storage_label = "Storage";
    let mut storage_ok = false;
    match &routed {
        None => steps.push(Step::skipped("storage", storage_label)),
        Some((r, key)) => match panel.buckets.iter().find(|b| b.id == r.dest.storage.id) {
            None => steps.push(Step::new("storage", storage_label, Status::Skip, "Configurazione scritta a mano: la verifica dello storage dal pannello non è disponibile.")),
            Some(b) => match bucket_test_req(admin, b) {
                Err(e) => steps.push(Step::new("storage", storage_label, Status::Fail, format!("Credenziali del bucket non leggibili: {e}")).fix("Rimuovi e ricrea il bucket inserendo di nuovo le chiavi.")),
                Ok(mut t) => {
                    t.prefix = String::new();
                    t.file = key.clone();
                    let t0 = Instant::now();
                    let out = storage_test(&t).await;
                    let ms = t0.elapsed().as_millis();
                    let outcome = out["outcome"].as_str().unwrap_or("invalid");
                    if outcome == "found" {
                        storage_ok = true;
                        let size = out["size"].as_str().filter(|s| !s.is_empty()).map(|s| format!(", {s} byte")).unwrap_or_default();
                        let ct = out["content_type"].as_str().filter(|s| !s.is_empty()).map(|s| format!(", {s}")).unwrap_or_default();
                        steps.push(Step::new("storage", storage_label, Status::Ok, format!("«{}» è leggibile su {} ({ms} ms{size}{ct}).", key, b.name)));
                    } else {
                        let msg = out["message"].as_str().unwrap_or("errore").to_owned();
                        steps.push(Step::new("storage", storage_label, Status::Fail, msg).fix(crate::diag::storage_fix(outcome)));
                    }
                }
            },
        },
    }

    // 5. risposta reale del nodo (può riempire la cache, come una visita vera)
    let resp_label = "Risposta del nodo";
    if routed.is_none() {
        steps.push(Step::skipped("response", resp_label));
    } else if let Ok(client) = s3::build_client(true) {
        let url = format!(
            "http://127.0.0.1:{}{}{}",
            admin.listen_port,
            raw_path,
            own_query(admin, &host, raw_path)
        );
        let t0 = Instant::now();
        match client.get(&url).header(header::HOST, &host).send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let xc = resp
                    .headers()
                    .get("x-cache")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("—")
                    .to_owned();
                let ms = t0.elapsed().as_millis();
                let ok = status < 300;
                let mut s = Step::new(
                    "response",
                    resp_label,
                    if ok { Status::Ok } else { Status::Fail },
                    format!("HTTP {status}, X-Cache: {xc}, {ms} ms."),
                );
                if !ok {
                    s = s.fix(if storage_ok { "Lo storage risponde ma il nodo no: guarda i log del nodo (RUST_LOG=otterroute=debug)." } else { "Vedi il passaggio «Storage» qui sopra." });
                }
                steps.push(s);
            }
            Err(e) => steps.push(
                Step::new(
                    "response",
                    resp_label,
                    Status::Fail,
                    format!("Richiesta al nodo non riuscita: {e}"),
                )
                .fix("Il nodo non risponde sulla sua porta pubblica: controlla OTR_LISTEN."),
            ),
        }
    } else {
        steps.push(Step::skipped("response", resp_label));
    }

    let when = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let node = format!(
        "nodo {}",
        admin.state.node_id.chars().take(8).collect::<String>()
    );
    let report = crate::diag::report(
        &format!("{}://{host}{raw_path}", parsed.scheme()),
        &node,
        &when,
        &steps,
    );
    audit::log("diagnose", &format!("{host}{raw_path}"));
    json(
        StatusCode::OK,
        json!({ "steps": steps, "summary": crate::diag::summary(&steps), "report": report }),
    )
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
    let url = format!(
        "http://127.0.0.1:{}{}{}",
        admin.listen_port,
        req.path,
        own_query(admin, &host, &req.path)
    );
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

    #[test]
    fn admin_gate_matches_only_its_host() {
        let gate = AdminGate::default();
        let req = |host: &str| {
            Request::builder()
                .uri("/x")
                .header("host", host)
                .body(())
                .unwrap()
        };
        assert!(
            !gate.is_admin(&req("pannello.example.com")),
            "spento: nessun dominio"
        );
        gate.set(Some("pannello.example.com".into()));
        assert!(gate.is_admin(&req("pannello.example.com")));
        assert!(
            gate.is_admin(&req("Pannello.Example.com:8443")),
            "maiuscole e porta non contano"
        );
        assert!(!gate.is_admin(&req("altro.example.com")));
        assert!(
            !gate.is_admin(&req("pannello.example.com.evil.it")),
            "niente corrispondenze parziali"
        );
        assert!(!gate.is_admin(&req("x.pannello.example.com")));
        let no_host = Request::builder().uri("/x").body(()).unwrap();
        assert!(!gate.is_admin(&no_host));
        gate.set(None);
        assert!(!gate.is_admin(&req("pannello.example.com")));
    }

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
