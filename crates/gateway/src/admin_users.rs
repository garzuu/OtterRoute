//! API per utenti, profilo personale (password e 2FA), criterio di sicurezza e
//! registro attività. Le autorizzazioni (scope) sono già state controllate da
//! `admin::api` con la tabella `admin::access`.

// Le risposte HTTP sono usate come tipo d'errore dei controlli interni.
#![allow(clippy::result_large_err)]

use http::{Method, Request, Response, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::admin::{bad, error, json, read_json, Admin};
use crate::audit;
use crate::auth::Principal;
use crate::body::Body;
use crate::totp;
use crate::users::{self, Policy, User};

/// Dati pubblici di un utente (mai hash, segreti o codici di recupero).
pub fn user_json(u: &User) -> Value {
    json!({
        "id": u.id,
        "username": u.username,
        "role": u.role,
        "scopes": u.effective_scopes(),
        "custom_scopes": u.scopes,
        "disabled": u.disabled,
        "two_factor": u.totp.enabled,
        "must_change_password": u.must_change_password,
        "created_at": u.created_at,
        "last_login_at": u.last_login_at,
    })
}

pub async fn handle(
    admin: &Admin,
    p: &Principal,
    method: &Method,
    path: &str,
    req: Request<hyper::body::Incoming>,
) -> Response<Body> {
    macro_rules! with_body {
        ($t:ty, $f:expr) => {
            match read_json::<$t>(req).await {
                Ok(b) => $f(admin, p, b).await,
                Err(r) => r,
            }
        };
    }
    let rest = path.strip_prefix("/api/users/");
    match (method, path) {
        (&Method::GET, "/api/users") => list_users(admin),
        (&Method::POST, "/api/users") => with_body!(NewUser, create_user),
        (&Method::GET, "/api/audit") => audit_list(req.uri().query()),
        (&Method::PUT, "/api/policy") => with_body!(PolicyReq, set_policy),
        (&Method::GET, "/api/me") => me(admin, p),
        (&Method::PUT, "/api/me/password") => with_body!(PasswordReq, change_password),
        (&Method::POST, "/api/me/2fa/start") => two_factor_start(admin, p),
        (&Method::POST, "/api/me/2fa/confirm") => with_body!(CodeReq, two_factor_confirm),
        (&Method::POST, "/api/me/2fa/disable") => with_body!(DisableReq, two_factor_disable),
        (&Method::POST, "/api/me/2fa/recovery") => with_body!(RecoveryReq, two_factor_recovery),
        (&Method::PUT, _) if rest.is_some() => {
            let id = rest.unwrap_or_default().to_owned();
            match read_json::<UpdateUser>(req).await {
                Ok(b) => update_user(admin, p, &id, b).await,
                Err(r) => r,
            }
        }
        (&Method::DELETE, _) if rest.is_some() => delete_user(admin, p, rest.unwrap_or_default()),
        (&Method::POST, _) if rest.is_some_and(|r| r.ends_with("/reset-password")) => {
            reset_password(
                admin,
                p,
                rest.unwrap_or_default().trim_end_matches("/reset-password"),
            )
            .await
        }
        (&Method::POST, _) if rest.is_some_and(|r| r.ends_with("/reset-2fa")) => reset_two_factor(
            admin,
            p,
            rest.unwrap_or_default().trim_end_matches("/reset-2fa"),
        ),
        _ => error(StatusCode::NOT_FOUND, "not found"),
    }
}

// ---------------------------------------------------------------------------
// Utenti (users:manage)
// ---------------------------------------------------------------------------

fn list_users(admin: &Admin) -> Response<Body> {
    let mut list = admin.auth.users.list();
    list.sort_by_key(|u| u.username.to_lowercase());
    json(
        StatusCode::OK,
        json!({
            "users": list.iter().map(user_json).collect::<Vec<_>>(),
            "scopes": users::SCOPES.iter().map(|(k, d)| json!({"id": k, "description": d})).collect::<Vec<_>>(),
            "policy": admin.auth.users.policy(),
        }),
    )
}

#[derive(Deserialize)]
struct NewUser {
    username: String,
    #[serde(default)]
    password: Option<String>,
    role: String,
    #[serde(default)]
    scopes: Vec<String>,
}

fn check_role(role: &str, scopes: &[String]) -> Result<(), String> {
    if !users::is_role(role) {
        return Err(format!("ruolo '{role}' sconosciuto"));
    }
    if let Some(bad) = scopes.iter().find(|s| !users::is_scope(s)) {
        return Err(format!("scope '{bad}' sconosciuto"));
    }
    Ok(())
}

async fn create_user(admin: &Admin, p: &Principal, req: NewUser) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let username = match users::check_username(&req.username) {
        Ok(u) => u,
        Err(e) => return bad(e),
    };
    if let Err(e) = check_role(&req.role, &req.scopes) {
        return bad(e);
    }
    // senza password indicata se ne genera una temporanea: l'utente dovrà cambiarla
    let password = req
        .password
        .filter(|s| !s.is_empty())
        .unwrap_or_else(users::temp_password);
    if let Err(e) = users::check_password(&password) {
        return bad(e);
    }
    let pw = password.clone();
    let hash = match tokio::task::spawn_blocking(move || users::hash_password(&pw)).await {
        Ok(Ok(h)) => h,
        _ => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "impossibile calcolare l'hash",
            )
        }
    };
    let user = User {
        id: users::new_id(),
        username,
        password_hash: hash,
        role: req.role.clone(),
        scopes: if req.role == "custom" {
            req.scopes
        } else {
            vec![]
        },
        disabled: false,
        must_change_password: true,
        totp: Default::default(),
        recovery_hashes: vec![],
        created_at: users::now_iso(),
        last_login_at: None,
    };
    match admin.auth.users.insert(user) {
        Ok(u) => {
            audit::log("user.create", &format!("{} ({})", u.username, u.role));
            let _ = p;
            json(
                StatusCode::OK,
                json!({"user": user_json(&u), "password": password}),
            )
        }
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

#[derive(Deserialize)]
struct UpdateUser {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    scopes: Option<Vec<String>>,
    #[serde(default)]
    disabled: Option<bool>,
}

async fn update_user(admin: &Admin, p: &Principal, id: &str, req: UpdateUser) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if req.disabled == Some(true) && id == p.id {
        return bad("non puoi disabilitare il tuo stesso account");
    }
    if let Some(role) = &req.role {
        if let Err(e) = check_role(role, req.scopes.as_deref().unwrap_or(&[])) {
            return bad(e);
        }
    } else if let Some(s) = &req.scopes {
        if let Err(e) = check_role("custom", s) {
            return bad(e);
        }
    }
    let result = admin.auth.users.update(id, |u| {
        if let Some(r) = &req.role {
            u.role = r.clone();
        }
        if let Some(s) = &req.scopes {
            u.scopes = s.clone();
        }
        if u.role != "custom" {
            u.scopes.clear();
        }
        if let Some(d) = req.disabled {
            u.disabled = d;
        }
        Ok(())
    });
    match result {
        Ok(u) => {
            if u.disabled {
                admin.auth.revoke_user_sessions(&u.id, None);
            }
            audit::log(
                "user.update",
                &format!(
                    "{} ({}{})",
                    u.username,
                    u.role,
                    if u.disabled { ", disabilitato" } else { "" }
                ),
            );
            json(StatusCode::OK, user_json(&u))
        }
        Err(e) if e == "utente non trovato" => error(StatusCode::NOT_FOUND, e),
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

fn delete_user(admin: &Admin, p: &Principal, id: &str) -> Response<Body> {
    if id == p.id {
        return bad("non puoi eliminare il tuo stesso account");
    }
    let name = admin.auth.users.get(id).map(|u| u.username);
    match admin.auth.users.delete(id) {
        Ok(()) => {
            admin.auth.revoke_user_sessions(id, None);
            audit::log("user.delete", name.as_deref().unwrap_or(id));
            json(StatusCode::OK, json!({"ok": true}))
        }
        Err(e) if e == "utente non trovato" => error(StatusCode::NOT_FOUND, e),
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

async fn reset_password(admin: &Admin, p: &Principal, id: &str) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if id == p.id {
        return bad("per cambiare la tua password usa il profilo");
    }
    let temp = users::temp_password();
    let t = temp.clone();
    let Ok(Ok(hash)) = tokio::task::spawn_blocking(move || users::hash_password(&t)).await else {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "impossibile calcolare l'hash",
        );
    };
    match admin.auth.users.update(id, |u| {
        u.password_hash = hash;
        u.must_change_password = true;
        Ok(())
    }) {
        Ok(u) => {
            admin.auth.revoke_user_sessions(&u.id, None);
            audit::log("user.reset-password", &u.username);
            json(StatusCode::OK, json!({"password": temp}))
        }
        Err(e) if e == "utente non trovato" => error(StatusCode::NOT_FOUND, e),
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

fn reset_two_factor(admin: &Admin, _p: &Principal, id: &str) -> Response<Body> {
    match admin.auth.users.update(id, |u| {
        u.totp = Default::default();
        u.recovery_hashes.clear();
        Ok(())
    }) {
        Ok(u) => {
            admin.auth.revoke_user_sessions(&u.id, None);
            audit::log("user.reset-2fa", &u.username);
            json(StatusCode::OK, user_json(&u))
        }
        Err(e) if e == "utente non trovato" => error(StatusCode::NOT_FOUND, e),
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

// ---------------------------------------------------------------------------
// Criterio e registro (users:manage)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PolicyReq {
    require_2fa: String,
}

async fn set_policy(admin: &Admin, _p: &Principal, req: PolicyReq) -> Response<Body> {
    if !Policy::is_valid(&req.require_2fa) {
        return bad("criterio non valido: usa off, all o managers");
    }
    let _g = admin.write_lock.lock().await;
    match admin.auth.users.set_policy(Policy {
        require_2fa: req.require_2fa.clone(),
    }) {
        Ok(()) => {
            audit::log("policy.update", &format!("2fa: {}", req.require_2fa));
            json(StatusCode::OK, json!({"require_2fa": req.require_2fa}))
        }
        Err(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

fn audit_list(query: Option<&str>) -> Response<Body> {
    let q = |name: &str| {
        query
            .unwrap_or_default()
            .split('&')
            .find_map(|kv| kv.strip_prefix(&format!("{name}=")))
            .map(str::to_owned)
    };
    let limit = q("limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    let before = q("before");
    let entries = audit::global()
        .map(|a| a.tail(limit, before.as_deref()))
        .unwrap_or_default();
    json(StatusCode::OK, json!({ "entries": entries }))
}

// ---------------------------------------------------------------------------
// Profilo personale
// ---------------------------------------------------------------------------

fn me(admin: &Admin, p: &Principal) -> Response<Body> {
    match admin.auth.users.get(&p.id) {
        Some(u) => json(
            StatusCode::OK,
            json!({"user": user_json(&u), "policy": admin.auth.users.policy()}),
        ),
        None => error(StatusCode::NOT_FOUND, "utente non trovato"),
    }
}

/// Riautenticazione per le azioni delicate: password giusta e non bloccati.
async fn reauth(admin: &Admin, p: &Principal, password: &str) -> Result<User, Response<Body>> {
    if let Some(s) = admin.auth.locked_for(&p.username) {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            format!("troppi tentativi: riprova tra {} minuti", s.div_ceil(60)),
        ));
    }
    let Some(user) = admin.auth.users.get(&p.id) else {
        return Err(error(StatusCode::NOT_FOUND, "utente non trovato"));
    };
    let (pw, hash) = (password.to_owned(), user.password_hash.clone());
    let ok = tokio::task::spawn_blocking(move || users::verify_password(&pw, &hash))
        .await
        .unwrap_or(false);
    if !ok {
        admin.auth.note_fail(&p.username);
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        return Err(error(StatusCode::UNAUTHORIZED, "password errata"));
    }
    Ok(user)
}

#[derive(Deserialize)]
struct PasswordReq {
    current: String,
    new: String,
}

async fn change_password(admin: &Admin, p: &Principal, req: PasswordReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    if let Err(r) = reauth(admin, p, &req.current).await {
        return r;
    }
    if let Err(e) = users::check_password(&req.new) {
        return bad(e);
    }
    if req.new == req.current {
        return bad("la nuova password deve essere diversa da quella attuale");
    }
    let pw = req.new.clone();
    let Ok(Ok(hash)) = tokio::task::spawn_blocking(move || users::hash_password(&pw)).await else {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "impossibile calcolare l'hash",
        );
    };
    match admin.auth.users.update(&p.id, |u| {
        u.password_hash = hash;
        u.must_change_password = false;
        Ok(())
    }) {
        Ok(_) => {
            // le altre sessioni (magari di chi conosceva la vecchia password) si chiudono
            admin.auth.revoke_user_sessions(&p.id, Some(&p.token));
            admin.auth.note_success(&p.username);
            audit::log("password.change", &p.username);
            json(StatusCode::OK, json!({"ok": true}))
        }
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

fn two_factor_start(admin: &Admin, p: &Principal) -> Response<Body> {
    let Some(user) = admin.auth.users.get(&p.id) else {
        return error(StatusCode::NOT_FOUND, "utente non trovato");
    };
    if user.totp.enabled {
        return error(
            StatusCode::CONFLICT,
            "la 2FA è già attiva: disattivala prima di riconfigurarla",
        );
    }
    let secret = totp::generate_secret();
    let s = secret.clone();
    if let Err(e) = admin.auth.users.update(&p.id, |u| {
        u.totp = users::Totp {
            secret: Some(s),
            enabled: false,
            last_step: 0,
        };
        Ok(())
    }) {
        return error(StatusCode::CONFLICT, e);
    }
    let url = totp::otpauth_url("OtterRoute", &user.username, &secret);
    json(
        StatusCode::OK,
        json!({"secret": secret, "otpauth_url": url, "qr_svg": totp::qr_svg(&url)}),
    )
}

#[derive(Deserialize)]
struct CodeReq {
    code: String,
}

async fn two_factor_confirm(admin: &Admin, p: &Principal, req: CodeReq) -> Response<Body> {
    let Some(user) = admin.auth.users.get(&p.id) else {
        return error(StatusCode::NOT_FOUND, "utente non trovato");
    };
    if user.totp.enabled {
        return error(StatusCode::CONFLICT, "la 2FA è già attiva");
    }
    let Some(secret) = user.totp.secret.clone() else {
        return bad("avvia prima la configurazione della 2FA");
    };
    if let Some(s) = admin.auth.locked_for(&p.username) {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            format!("troppi tentativi: riprova tra {} minuti", s.div_ceil(60)),
        );
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let Some(step) = totp::verify(&secret, &req.code, now, 0) else {
        admin.auth.note_fail(&p.username);
        audit::log_as(&p.username, "2fa.confirm-failed", "", false);
        return bad("codice errato: controlla l'ora del telefono e riprova");
    };
    let codes = totp::generate_recovery_codes();
    let hashes: Vec<String> = codes.iter().map(|c| totp::hash_recovery_code(c)).collect();
    match admin.auth.users.update(&p.id, |u| {
        u.totp.enabled = true;
        u.totp.last_step = step;
        u.recovery_hashes = hashes;
        Ok(())
    }) {
        Ok(_) => {
            audit::log("2fa.enable", &p.username);
            json(StatusCode::OK, json!({"recovery_codes": codes}))
        }
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

#[derive(Deserialize)]
struct DisableReq {
    password: String,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    recovery_code: Option<String>,
}

async fn two_factor_disable(admin: &Admin, p: &Principal, req: DisableReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let user = match reauth(admin, p, &req.password).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !user.totp.enabled {
        return error(StatusCode::CONFLICT, "la 2FA non è attiva");
    }
    if admin.auth.users.policy().requires(&user) {
        return error(
            StatusCode::FORBIDDEN,
            "il criterio di sicurezza richiede la 2FA per il tuo account",
        );
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let by_code = req
        .code
        .as_deref()
        .zip(user.totp.secret.as_deref())
        .is_some_and(|(c, s)| totp::verify(s, c, now, user.totp.last_step).is_some());
    let by_recovery = req
        .recovery_code
        .as_deref()
        .is_some_and(|rc| user.recovery_hashes.contains(&totp::hash_recovery_code(rc)));
    if !by_code && !by_recovery {
        admin.auth.note_fail(&p.username);
        return bad("serve un codice valido dell'app di autenticazione (o un codice di recupero)");
    }
    match admin.auth.users.update(&p.id, |u| {
        u.totp = Default::default();
        u.recovery_hashes.clear();
        Ok(())
    }) {
        Ok(_) => {
            audit::log("2fa.disable", &p.username);
            json(StatusCode::OK, json!({"ok": true}))
        }
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}

#[derive(Deserialize)]
struct RecoveryReq {
    password: String,
}

async fn two_factor_recovery(admin: &Admin, p: &Principal, req: RecoveryReq) -> Response<Body> {
    let _g = admin.write_lock.lock().await;
    let user = match reauth(admin, p, &req.password).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !user.totp.enabled {
        return error(StatusCode::CONFLICT, "la 2FA non è attiva");
    }
    let codes = totp::generate_recovery_codes();
    let hashes: Vec<String> = codes.iter().map(|c| totp::hash_recovery_code(c)).collect();
    match admin.auth.users.update(&p.id, |u| {
        u.recovery_hashes = hashes;
        Ok(())
    }) {
        Ok(_) => {
            audit::log("2fa.recovery-codes", &p.username);
            json(StatusCode::OK, json!({"recovery_codes": codes}))
        }
        Err(e) => error(StatusCode::CONFLICT, e),
    }
}
