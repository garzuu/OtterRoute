mod body;
mod cache;
mod config;
mod duration;
mod handler;
mod routing;
mod s3;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use arc_swap::ArcSwap;
use clap::Parser;
use http::{header, HeaderValue, Response, StatusCode};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::net::TcpListener;

use crate::cache::Cache;
use crate::handler::AppState;

#[derive(Parser, Debug)]
#[command(
    name = "otterroute",
    version,
    about = "OtterRoute gateway: pubblica oggetti S3 su più domini"
)]
struct Args {
    /// Configurazione pubblicata (YAML)
    #[arg(long, env = "OTR_CONFIG", default_value = "config.yaml")]
    config: PathBuf,
    /// Indirizzo del traffico pubblico
    #[arg(long, env = "OTR_LISTEN", default_value = "0.0.0.0:8080")]
    listen: SocketAddr,
    /// Indirizzo dell'endpoint di amministrazione (salute, stato)
    #[arg(long, env = "OTR_ADMIN_LISTEN", default_value = "127.0.0.1:9090")]
    admin_listen: SocketAddr,
    /// Cartella della cache
    #[arg(long, env = "OTR_CACHE_DIR", default_value = "./data/cache")]
    cache_dir: PathBuf,
    /// Spazio massimo della cache in byte
    #[arg(long, env = "OTR_CACHE_MAX_BYTES", default_value_t = 10 * 1024 * 1024 * 1024)]
    cache_max_bytes: u64,
    /// Oggetti più grandi vengono serviti senza cache
    #[arg(long, env = "OTR_CACHE_MAX_OBJECT_BYTES", default_value_t = 1024 * 1024 * 1024)]
    cache_max_object_bytes: u64,
    /// Cartella di stato (ultima configurazione valida)
    #[arg(long, env = "OTR_STATE_DIR", default_value = "./data/state")]
    state_dir: PathBuf,
    /// Ogni quanto controllare se la configurazione è cambiata
    #[arg(long, env = "OTR_RELOAD_INTERVAL", default_value = "2s", value_parser = duration::parse)]
    reload_interval: Duration,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "otterroute=info".into()),
        )
        .init();

    let args = Args::parse();
    std::fs::create_dir_all(&args.state_dir).context("state_dir")?;
    let last_good = args.state_dir.join("last-good.yaml");

    let snapshot = initial_snapshot(&args.config, &last_good)?;
    tracing::info!(
        version = snapshot.version,
        routes = snapshot.route_count(),
        "configurazione attiva"
    );

    let cache = Cache::open(&args.cache_dir, args.cache_max_bytes).context("apertura cache")?;
    let state = Arc::new(AppState {
        snapshot: ArcSwap::from_pointee(snapshot),
        cache,
        max_object_bytes: args.cache_max_object_bytes,
    });

    tokio::spawn(watch_config(
        state.clone(),
        args.config.clone(),
        last_good,
        args.reload_interval,
    ));

    let admin = TcpListener::bind(args.admin_listen)
        .await
        .context("bind admin")?;
    tracing::info!(addr = %args.admin_listen, "admin in ascolto");
    let st = state.clone();
    tokio::spawn(serve(admin, move |req| {
        let st = st.clone();
        async move { Ok::<_, std::convert::Infallible>(admin_handler(&st, req)) }
    }));

    let public = TcpListener::bind(args.listen)
        .await
        .context("bind pubblico")?;
    tracing::info!(addr = %args.listen, "gateway in ascolto");
    let st = state.clone();
    let server = serve(public, move |req| handler::handle(st.clone(), req));

    tokio::select! {
        _ = server => {}
        _ = shutdown_signal() => tracing::info!("arresto"),
    }
    Ok(())
}

/// Configurazione all'avvio: il file indicato, oppure l'ultima valida salvata.
/// Così un nodo che riparte mentre il pannello è giù continua a servire.
fn initial_snapshot(config: &Path, last_good: &Path) -> anyhow::Result<config::Snapshot> {
    match std::fs::read(config)
        .map_err(anyhow::Error::from)
        .and_then(|raw| {
            let s = config::parse(&raw)?;
            save_last_good(last_good, &raw);
            Ok(s)
        }) {
        Ok(s) => Ok(s),
        Err(e) => {
            tracing::error!(error = %e, "configurazione non utilizzabile, provo l'ultima valida");
            let raw =
                std::fs::read(last_good).context("nessuna configurazione valida disponibile")?;
            config::parse(&raw).context("anche l'ultima configurazione valida è inutilizzabile")
        }
    }
}

fn save_last_good(path: &Path, raw: &[u8]) {
    let tmp = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp, raw).and_then(|_| std::fs::rename(&tmp, path)) {
        tracing::warn!(error = %e, "impossibile salvare l'ultima configurazione valida");
    }
}

/// Controlla periodicamente il file. La nuova configurazione sostituisce la
/// vecchia in blocco: le richieste in corso finiscono con quella con cui sono
/// partite, nessun download viene interrotto.
async fn watch_config(state: Arc<AppState>, path: PathBuf, last_good: PathBuf, every: Duration) {
    let stamp = |p: &Path| -> Option<(SystemTime, u64)> {
        let m = std::fs::metadata(p).ok()?;
        Some((m.modified().ok()?, m.len()))
    };
    let mut last = stamp(&path);
    let mut tick = tokio::time::interval(every);
    loop {
        tick.tick().await;
        let now = stamp(&path);
        if now.is_none() || now == last {
            continue;
        }
        last = now;
        let raw = match tokio::fs::read(&path).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "lettura configurazione");
                continue;
            }
        };
        match config::parse(&raw) {
            Ok(snap) => {
                let current = state.snapshot.load().version;
                if snap.version <= current {
                    tracing::warn!(
                        new = snap.version,
                        current,
                        "configurazione ignorata: version deve aumentare"
                    );
                    continue;
                }
                tracing::info!(
                    from = current,
                    to = snap.version,
                    routes = snap.route_count(),
                    "configurazione applicata"
                );
                state.snapshot.store(Arc::new(snap));
                save_last_good(&last_good, &raw);
            }
            Err(e) => {
                tracing::error!(error = %e, "configurazione rifiutata, resta attiva la precedente")
            }
        }
    }
}

fn admin_handler(
    state: &AppState,
    req: http::Request<hyper::body::Incoming>,
) -> Response<body::Body> {
    let json = |status: StatusCode, v: serde_json::Value| {
        let mut r = Response::new(body::full(v.to_string()));
        *r.status_mut() = status;
        r.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        r
    };
    match req.uri().path() {
        "/healthz" => json(StatusCode::OK, serde_json::json!({"status": "ok"})),
        "/status" => {
            let snap = state.snapshot.load();
            let mut routes: Vec<_> = snap
                .routes_by_host
                .values()
                .flatten()
                .map(|r| {
                    serde_json::json!({
                        "id": r.id,
                        "match": format!("{}{}", r.host, r.path_prefix),
                        "destination": r.dest.id,
                        "cache_policy": r.policy.id,
                        "storage": r.dest.storage.id,
                        "bucket": r.dest.bucket,
                        "prefix": r.dest.prefix,
                        "cache_generation": r.dest.cache_generation,
                    })
                })
                .collect();
            routes.sort_by(|a, b| a["match"].as_str().cmp(&b["match"].as_str()));
            json(
                StatusCode::OK,
                serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "config_version": snap.version,
                    "routes": routes,
                    "cache": state.cache.stats(),
                }),
            )
        }
        _ => json(
            StatusCode::NOT_FOUND,
            serde_json::json!({"error": "not found"}),
        ),
    }
}

async fn serve<F, Fut>(listener: TcpListener, f: F)
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
                tracing::warn!(error = %e, "accept");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };
        let _ = stream.set_nodelay(true);
        let f = f.clone();
        tokio::spawn(async move {
            let svc = service_fn(f);
            if let Err(e) = auto::Builder::new(TokioExecutor::new())
                .serve_connection(TokioIo::new(stream), svc)
                .await
            {
                tracing::debug!(error = %e, "connessione chiusa");
            }
        });
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
}
