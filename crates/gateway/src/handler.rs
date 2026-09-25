//! Gestione di una richiesta pubblica: regola → cache → storage.

use std::convert::Infallible;
use std::io::{self, SeekFrom};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use arc_swap::ArcSwap;
use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt};
use http::{header, HeaderMap, HeaderValue, Method, Response, StatusCode};
use hyper::body::Incoming;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::mpsc;
use tokio_util::io::ReaderStream;

use crate::body::{self, Body};
use crate::cache::{Cache, CacheWriter, Entry, FillGuard, FillRole};
use crate::config::{CachePolicy, Route, Snapshot};
use crate::routing::{cache_key, cache_query, normalize_path, request_host};
use crate::s3::{self, Fetch};

/// Quanto aspetta una richiesta se lo stesso oggetto è già in download.
const COALESCE_WAIT: Duration = Duration::from_secs(5);

/// Header dello storage che inoltriamo al client. Tutto il resto (x-amz-*,
/// metadati utente, server, ...) resta interno.
const FORWARDED_HEADERS: &[&str] = &[
    "content-type",
    "content-length",
    "content-range",
    "content-encoding",
    "content-language",
    "content-disposition",
    "etag",
    "last-modified",
];

pub struct AppState {
    pub snapshot: ArcSwap<Snapshot>,
    pub cache: Arc<Cache>,
    pub max_object_bytes: u64,
    /// identità del nodo, usata per dimostrare che risponde lui su un dominio
    pub node_id: String,
    /// contatori di traffico (dashboard e /metrics)
    pub metrics: Arc<crate::metrics::Metrics>,
}

struct Ctx {
    is_head: bool,
    range: Option<String>,
    if_range: Option<String>,
    if_none_match: Option<String>,
    if_modified_since: Option<SystemTime>,
}

impl Ctx {
    fn from_headers(method: &Method, h: &HeaderMap) -> Self {
        let get = |n: header::HeaderName| h.get(n).and_then(|v| v.to_str().ok()).map(str::to_owned);
        Ctx {
            is_head: method == Method::HEAD,
            range: get(header::RANGE),
            if_range: get(header::IF_RANGE),
            if_none_match: get(header::IF_NONE_MATCH),
            if_modified_since: get(header::IF_MODIFIED_SINCE)
                .and_then(|s| httpdate::parse_http_date(&s).ok()),
        }
    }

    /// RFC 9110: If-None-Match ha la precedenza su If-Modified-Since.
    fn not_modified(&self, etag: Option<&str>, last_modified: Option<&str>) -> bool {
        if let Some(inm) = &self.if_none_match {
            let Some(etag) = etag else { return false };
            let strip = |s: &str| s.trim().trim_start_matches("W/").to_owned();
            return inm.trim() == "*" || inm.split(',').any(|t| strip(t) == strip(etag));
        }
        match (
            self.if_modified_since,
            last_modified.and_then(|s| httpdate::parse_http_date(s).ok()),
        ) {
            (Some(ims), Some(lm)) => lm <= ims,
            _ => false,
        }
    }

    /// Range da applicare a un oggetto in cache (ignorato se If-Range non corrisponde).
    fn effective_range(&self, etag: Option<&str>, last_modified: Option<&str>) -> Option<&str> {
        let r = self.range.as_deref()?;
        if let Some(ir) = &self.if_range {
            let ok = Some(ir.as_str()) == etag || Some(ir.as_str()) == last_modified;
            if !ok {
                return None;
            }
        }
        Some(r)
    }
}

#[derive(Debug)]
struct HttpError(StatusCode, &'static str);

impl HttpError {
    fn into_response(self) -> Response<Body> {
        let mut r = Response::new(body::full(format!("{}\n", self.1)));
        *r.status_mut() = self.0;
        r.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        r.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if self.0 == StatusCode::METHOD_NOT_ALLOWED {
            r.headers_mut()
                .insert(header::ALLOW, HeaderValue::from_static("GET, HEAD"));
        }
        r
    }
}

const NOT_FOUND: HttpError = HttpError(StatusCode::NOT_FOUND, "not found");
const BAD_GATEWAY: HttpError = HttpError(StatusCode::BAD_GATEWAY, "bad gateway");

pub async fn handle(
    state: Arc<AppState>,
    req: http::Request<Incoming>,
) -> Result<Response<Body>, Infallible> {
    let t0 = Instant::now();
    let (parts, _) = req.into_parts();
    let tag = Arc::new(crate::metrics::ReqTag::default());
    let resp = crate::metrics::scope(tag.clone(), async {
        match handle_inner(&state, &parts).await {
            Ok(r) => r,
            Err(e) => e.into_response(),
        }
    })
    .await;
    let ms = t0.elapsed().as_millis() as u64;
    let cache = resp
        .headers()
        .get("x-cache")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    tracing::info!(
        method = %parts.method,
        host = parts.headers.get(header::HOST).and_then(|v| v.to_str().ok()).unwrap_or("-"),
        path = parts.uri.path(),
        status = resp.status().as_u16(),
        cache = cache.as_deref().unwrap_or("-"),
        ms,
        "request"
    );
    // i controlli dei domini fatti dal pannello non sono traffico
    if parts.uri.path() == crate::dns::CHECK_PATH {
        return Ok(resp);
    }
    let sample = crate::metrics::RequestSample::new(
        &tag,
        parts.uri.path(),
        resp.status().as_u16(),
        cache.as_deref(),
        ms,
    );
    let (rp, body) = resp.into_parts();
    Ok(Response::from_parts(rp, state.metrics.track(body, sample)))
}

/// Risposta di verifica: la usa il pannello per controllare che un dominio
/// arrivi davvero a questo nodo. Vale per qualsiasi host, prima delle regole.
fn node_proof(state: &AppState, parts: &http::request::Parts) -> Response<Body> {
    let nonce: String = parts
        .uri
        .query()
        .unwrap_or_default()
        .split('&')
        .find_map(|kv| kv.strip_prefix("nonce="))
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(64)
        .collect();
    let body = serde_json::json!({
        "otterroute": true,
        "proof": crate::dns::proof(&state.node_id, &nonce),
    });
    let mut r = Response::new(body::full(body.to_string()));
    r.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

async fn handle_inner(
    state: &Arc<AppState>,
    parts: &http::request::Parts,
) -> Result<Response<Body>, HttpError> {
    if parts.method != Method::GET && parts.method != Method::HEAD {
        return Err(HttpError(
            StatusCode::METHOD_NOT_ALLOWED,
            "method not allowed",
        ));
    }
    if parts.uri.path() == crate::dns::CHECK_PATH {
        return Ok(node_proof(state, parts));
    }
    let host = request_host(parts).ok_or(HttpError(StatusCode::BAD_REQUEST, "bad host"))?;
    let path = normalize_path(parts.uri.path())
        .map_err(|_| HttpError(StatusCode::BAD_REQUEST, "bad path"))?;

    let snap = state.snapshot.load_full();
    let route = snap.match_route(&host, &path).ok_or(NOT_FOUND)?.clone();
    crate::metrics::note_route(&route.id);
    let key = route.object_key(&path).ok_or(NOT_FOUND)?;
    let ck = cache_key(
        &route,
        &key,
        &cache_query(parts.uri.query(), &route.policy.query_keys),
    );
    let ctx = Ctx::from_headers(&parts.method, &parts.headers);

    // 1. copia fresca in cache
    let stale = match state.cache.lookup(&ck).await {
        Some(e) if e.is_fresh(&route.policy) => {
            return serve_cached(e, &ctx, &route.policy, "HIT").await
        }
        other => other,
    };

    // 2. HEAD e richieste parziali senza copia fresca: passano allo storage
    //    senza riempire la cache (prototipo; vedi README → limiti noti)
    if ctx.is_head || ctx.range.is_some() {
        return passthrough(&route, &key, &parts.method, &ctx, stale).await;
    }

    // 3. GET completo: una sola richiesta per oggetto scarica e riempie la cache
    match state.cache.begin_fill(&ck) {
        FillRole::Follower(mut rx) => {
            let _ = tokio::time::timeout(COALESCE_WAIT, rx.changed()).await;
            if let Some(e) = state.cache.lookup(&ck).await {
                if e.is_fresh(&route.policy) {
                    return serve_cached(e, &ctx, &route.policy, "HIT").await;
                }
            }
            passthrough(&route, &key, &parts.method, &ctx, stale).await
        }
        FillRole::Leader(guard) => fetch_and_fill(state, route, key, ck, ctx, stale, guard).await,
    }
}

async fn fetch_and_fill(
    state: &Arc<AppState>,
    route: Arc<Route>,
    key: String,
    ck: String,
    ctx: Ctx,
    stale: Option<Entry>,
    guard: FillGuard,
) -> Result<Response<Body>, HttpError> {
    let d = &route.dest;
    let policy = &route.policy;

    // se abbiamo una copia scaduta con ETag, chiediamo allo storage se è cambiata
    let mut extra = Vec::new();
    if let Some(etag) = stale
        .as_ref()
        .filter(|s| s.meta.status == 200)
        .and_then(|s| s.meta.header("etag"))
    {
        extra.push(("if-none-match".to_string(), etag.to_string()));
    }

    match s3::fetch(&d.storage, &d.bucket, &key, &Method::GET, &extra).await {
        Fetch::NotModified => match stale {
            Some(mut s) => {
                if let Err(e) = state.cache.refresh(&ck, &s.meta).await {
                    tracing::warn!(error = %e, "refresh cache fallito");
                }
                s.meta.stored_at = crate::cache::now_secs();
                serve_cached(s, &ctx, policy, "REVALIDATED").await
            }
            None => Err(BAD_GATEWAY),
        },
        Fetch::Ok(resp) if resp.status() == StatusCode::OK => {
            let headers = forwarded(resp.headers());
            let len = resp.content_length();
            let cacheable = len.is_none_or(|l| l <= state.max_object_bytes);
            let etag = header_of(&headers, "etag");
            let lm = header_of(&headers, "last-modified");
            let client_304 = ctx.not_modified(etag.as_deref(), lm.as_deref());

            let writer = if cacheable {
                match state.cache.writer(&ck).await {
                    Ok(w) => Some(w),
                    Err(e) => {
                        tracing::warn!(error = %e, "impossibile scrivere in cache");
                        None
                    }
                }
            } else {
                None
            };

            if client_304 {
                if let Some(w) = writer {
                    tokio::spawn(pump(
                        resp,
                        Some(w),
                        None,
                        state.clone(),
                        headers.clone(),
                        len,
                        guard,
                    ));
                }
                return Ok(not_modified_response(&headers, policy, "MISS"));
            }

            let (tx, rx) = mpsc::channel::<io::Result<Bytes>>(8);
            let x_cache = if writer.is_some() { "MISS" } else { "BYPASS" };
            tokio::spawn(pump(
                resp,
                writer,
                Some(tx),
                state.clone(),
                headers.clone(),
                len,
                guard,
            ));

            let mut r = Response::new(body::channel(rx));
            apply_headers(&mut r, &headers, policy, x_cache, StatusCode::OK);
            Ok(r)
        }
        Fetch::Ok(resp) => {
            tracing::warn!(status = %resp.status(), "stato inatteso su GET completo");
            Err(BAD_GATEWAY)
        }
        Fetch::NotFound => {
            if !policy.ttl_not_found.is_zero() {
                if let Err(e) = state.cache.store_not_found(&ck).await {
                    tracing::warn!(error = %e, "cache negativa fallita");
                }
            }
            drop(guard);
            let mut r = NOT_FOUND.into_response();
            set_cache_headers(&mut r, policy.ttl_not_found, "MISS");
            Ok(r)
        }
        Fetch::Misconfigured(msg) => {
            crate::metrics::note_upstream_error(&d.storage.id);
            tracing::error!(route = %route.id, storage = %d.storage.id, %msg, "storage rifiuta le credenziali");
            Err(BAD_GATEWAY)
        }
        Fetch::Upstream(msg) => {
            crate::metrics::note_upstream_error(&d.storage.id);
            tracing::warn!(route = %route.id, storage = %d.storage.id, %msg, "storage non disponibile");
            match stale {
                Some(s) if s.is_usable_stale(policy) => {
                    serve_cached(s, &ctx, policy, "STALE").await
                }
                _ => Err(BAD_GATEWAY),
            }
        }
    }
}

/// Legge dallo storage, scrive in cache e (se c'è ancora) inoltra al client.
/// Se il client si disconnette il download continua per completare la cache.
async fn pump(
    resp: reqwest::Response,
    mut writer: Option<CacheWriter>,
    mut tx: Option<mpsc::Sender<io::Result<Bytes>>>,
    state: Arc<AppState>,
    headers: Vec<(String, String)>,
    expected: Option<u64>,
    _guard: FillGuard,
) {
    let mut stream = resp.bytes_stream();
    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                if let Some(w) = writer.as_mut() {
                    let too_big = w.written() + chunk.len() as u64 > state.max_object_bytes;
                    if too_big || w.write(&chunk).await.is_err() {
                        if let Some(w) = writer.take() {
                            w.abort().await;
                        }
                    }
                }
                if let Some(t) = &tx {
                    if t.send(Ok(chunk)).await.is_err() {
                        tx = None; // client andato via
                    }
                }
                if writer.is_none() && tx.is_none() {
                    return;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "download interrotto");
                if let Some(t) = tx {
                    let _ = t.send(Err(io::Error::other(e))).await;
                }
                if let Some(w) = writer {
                    w.abort().await;
                }
                return;
            }
        }
    }
    if let Some(w) = writer {
        if expected.is_none_or(|e| e == w.written()) {
            if let Err(e) = w.commit(&state.cache, 200, headers).await {
                tracing::warn!(error = %e, "commit cache fallito");
            }
        } else {
            w.abort().await;
        }
    }
}

/// Richiesta inoltrata allo storage così com'è (HEAD, Range), senza cache.
async fn passthrough(
    route: &Route,
    key: &str,
    method: &Method,
    ctx: &Ctx,
    stale: Option<Entry>,
) -> Result<Response<Body>, HttpError> {
    let d = &route.dest;
    let mut extra = Vec::new();
    if let Some(r) = &ctx.range {
        extra.push(("range".to_string(), r.clone()));
    }
    match s3::fetch(&d.storage, &d.bucket, key, method, &extra).await {
        Fetch::Ok(resp) => {
            let status = resp.status();
            let headers = forwarded(resp.headers());
            if status == StatusCode::OK
                && ctx.not_modified(
                    header_of(&headers, "etag").as_deref(),
                    header_of(&headers, "last-modified").as_deref(),
                )
            {
                return Ok(not_modified_response(&headers, &route.policy, "BYPASS"));
            }
            let b = if ctx.is_head {
                body::empty()
            } else {
                body::stream(resp.bytes_stream().map_err(io::Error::other))
            };
            let mut r = Response::new(b);
            apply_headers(&mut r, &headers, &route.policy, "BYPASS", status);
            Ok(r)
        }
        Fetch::NotFound => Err(NOT_FOUND),
        Fetch::NotModified => Err(BAD_GATEWAY),
        Fetch::Misconfigured(msg) => {
            crate::metrics::note_upstream_error(&d.storage.id);
            tracing::error!(route = %route.id, storage = %d.storage.id, %msg, "storage rifiuta le credenziali");
            Err(BAD_GATEWAY)
        }
        Fetch::Upstream(msg) => {
            crate::metrics::note_upstream_error(&d.storage.id);
            tracing::warn!(route = %route.id, storage = %d.storage.id, %msg, "storage non disponibile");
            match stale {
                Some(s) if s.is_usable_stale(&route.policy) => {
                    serve_cached(s, ctx, &route.policy, "STALE").await
                }
                _ => Err(BAD_GATEWAY),
            }
        }
    }
}

async fn serve_cached(
    e: Entry,
    ctx: &Ctx,
    policy: &CachePolicy,
    x_cache: &'static str,
) -> Result<Response<Body>, HttpError> {
    if e.meta.status == 404 {
        let mut r = NOT_FOUND.into_response();
        set_cache_headers(&mut r, policy.ttl_not_found, x_cache);
        return Ok(r);
    }
    let headers = &e.meta.headers;
    let etag = e.meta.header("etag");
    let lm = e.meta.header("last-modified");
    if ctx.not_modified(etag, lm) {
        return Ok(not_modified_response(headers, policy, x_cache));
    }

    let size = e.meta.size;
    let age = e.age().as_secs();
    let (status, start, len) = match ctx.effective_range(etag, lm).map(|r| parse_range(r, size)) {
        None | Some(RangeSpec::Full) => (StatusCode::OK, 0, size),
        Some(RangeSpec::Partial(s, end)) => (StatusCode::PARTIAL_CONTENT, s, end - s + 1),
        Some(RangeSpec::Unsatisfiable) => {
            let mut r = Response::new(body::empty());
            *r.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            r.headers_mut()
                .insert(header::CONTENT_RANGE, hv(&format!("bytes */{size}")));
            return Ok(r);
        }
    };

    let b = if ctx.is_head {
        body::empty()
    } else {
        let mut f = e.file.ok_or(BAD_GATEWAY)?;
        if start > 0 {
            f.seek(SeekFrom::Start(start))
                .await
                .map_err(|_| BAD_GATEWAY)?;
        }
        body::stream(ReaderStream::with_capacity(f.take(len), 64 * 1024))
    };

    let mut r = Response::new(b);
    apply_headers(&mut r, headers, policy, x_cache, status);
    r.headers_mut()
        .insert(header::CONTENT_LENGTH, hv(&len.to_string()));
    if status == StatusCode::PARTIAL_CONTENT {
        r.headers_mut().insert(
            header::CONTENT_RANGE,
            hv(&format!("bytes {start}-{}/{size}", start + len - 1)),
        );
    }
    r.headers_mut().insert(header::AGE, hv(&age.to_string()));
    Ok(r)
}

// ---------------------------------------------------------------------------
// Header e range
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum RangeSpec {
    Full,
    Partial(u64, u64),
    Unsatisfiable,
}

/// Un solo intervallo `bytes=a-b`, `bytes=a-`, `bytes=-n`. Più intervalli o
/// sintassi non valide → risposta completa (ammesso da RFC 9110).
fn parse_range(h: &str, size: u64) -> RangeSpec {
    let Some(spec) = h.trim().strip_prefix("bytes=") else {
        return RangeSpec::Full;
    };
    if spec.contains(',') {
        return RangeSpec::Full;
    }
    let Some((a, b)) = spec.split_once('-') else {
        return RangeSpec::Full;
    };
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        let Ok(n) = b.parse::<u64>() else {
            return RangeSpec::Full;
        };
        if n == 0 || size == 0 {
            return RangeSpec::Unsatisfiable;
        }
        return RangeSpec::Partial(size.saturating_sub(n), size - 1);
    }
    let Ok(start) = a.parse::<u64>() else {
        return RangeSpec::Full;
    };
    let end = if b.is_empty() {
        size.saturating_sub(1)
    } else {
        match b.parse::<u64>() {
            Ok(e) if e >= start => e.min(size.saturating_sub(1)),
            _ => return RangeSpec::Full,
        }
    };
    if start >= size {
        return RangeSpec::Unsatisfiable;
    }
    RangeSpec::Partial(start, end)
}

fn forwarded(h: &reqwest::header::HeaderMap) -> Vec<(String, String)> {
    FORWARDED_HEADERS
        .iter()
        .filter_map(|n| {
            h.get(*n)
                .and_then(|v| v.to_str().ok())
                .map(|v| (n.to_string(), v.to_owned()))
        })
        .collect()
}

fn header_of(h: &[(String, String)], name: &str) -> Option<String> {
    h.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
}

fn hv(s: &str) -> HeaderValue {
    HeaderValue::from_str(s).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn apply_headers(
    r: &mut Response<Body>,
    headers: &[(String, String)],
    policy: &CachePolicy,
    x_cache: &'static str,
    status: StatusCode,
) {
    *r.status_mut() = status;
    let h = r.headers_mut();
    for (k, v) in headers {
        if let (Ok(name), Ok(val)) = (
            header::HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(v),
        ) {
            h.insert(name, val);
        }
    }
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    set_cache_headers(r, policy.ttl, x_cache);
}

fn not_modified_response(
    headers: &[(String, String)],
    policy: &CachePolicy,
    x_cache: &'static str,
) -> Response<Body> {
    let mut r = Response::new(body::empty());
    *r.status_mut() = StatusCode::NOT_MODIFIED;
    for (k, v) in headers
        .iter()
        .filter(|(k, _)| k == "etag" || k == "last-modified")
    {
        if let (Ok(name), Ok(val)) = (
            header::HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(v),
        ) {
            r.headers_mut().insert(name, val);
        }
    }
    set_cache_headers(&mut r, policy.ttl, x_cache);
    r
}

fn set_cache_headers(r: &mut Response<Body>, ttl: Duration, x_cache: &'static str) {
    r.headers_mut().insert(
        header::CACHE_CONTROL,
        hv(&format!("public, max-age={}", ttl.as_secs())),
    );
    r.headers_mut()
        .insert("x-cache", HeaderValue::from_static(x_cache));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-9", 100), RangeSpec::Partial(0, 9));
        assert_eq!(parse_range("bytes=90-", 100), RangeSpec::Partial(90, 99));
        assert_eq!(parse_range("bytes=-10", 100), RangeSpec::Partial(90, 99));
        assert_eq!(parse_range("bytes=-500", 100), RangeSpec::Partial(0, 99));
        assert_eq!(
            parse_range("bytes=50-1000", 100),
            RangeSpec::Partial(50, 99)
        );
        assert_eq!(parse_range("bytes=100-", 100), RangeSpec::Unsatisfiable);
        assert_eq!(parse_range("bytes=0-1,5-6", 100), RangeSpec::Full);
        assert_eq!(parse_range("bytes=9-3", 100), RangeSpec::Full);
        assert_eq!(parse_range("items=0-1", 100), RangeSpec::Full);
    }

    fn ctx(inm: Option<&str>, ims: Option<&str>) -> Ctx {
        Ctx {
            is_head: false,
            range: None,
            if_range: None,
            if_none_match: inm.map(str::to_owned),
            if_modified_since: ims.and_then(|s| httpdate::parse_http_date(s).ok()),
        }
    }

    #[test]
    fn conditionals() {
        let lm = "Tue, 21 Oct 2025 07:28:00 GMT";
        assert!(ctx(Some("\"abc\""), None).not_modified(Some("\"abc\""), None));
        assert!(ctx(Some("W/\"abc\", \"x\""), None).not_modified(Some("\"abc\""), None));
        assert!(!ctx(Some("\"zzz\""), Some(lm)).not_modified(Some("\"abc\""), Some(lm)));
        assert!(ctx(None, Some(lm)).not_modified(None, Some(lm)));
        assert!(!ctx(None, Some("Wed, 21 Oct 2020 07:28:00 GMT")).not_modified(None, Some(lm)));
    }
}
