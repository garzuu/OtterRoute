mod acme;
mod admin;
mod admin_users;
mod audit;
mod auth;
mod body;
mod cache;
mod config;
mod diag;
mod dns;
#[cfg(test)]
mod docs_check;
mod duration;
mod handler;
mod metrics;
mod notify;
mod panel;
mod routing;
mod s3;
mod sign;
mod tls;
mod totp;
mod users;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Context;
use arc_swap::ArcSwap;
use clap::Parser;
use http::Response;
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
    /// Indirizzo del traffico pubblico (porta standard HTTP: 80)
    #[arg(long, env = "OTR_LISTEN", default_value = "0.0.0.0:80")]
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
    /// Ogni quanto ricontrollare i domini non ancora verificati o in errore
    #[arg(long, env = "OTR_DOMAIN_RECHECK", default_value = "5m", value_parser = duration::parse)]
    domain_recheck: Duration,
    /// Ogni quanto ricontrollare i domini già verificati
    #[arg(long, env = "OTR_DOMAIN_RECHECK_VERIFIED", default_value = "15m", value_parser = duration::parse)]
    domain_recheck_verified: Duration,
    /// Recupero accesso: imposta una password temporanea per l'utente, disattiva la
    /// sua 2FA e lo riabilita, poi esce senza avviare il nodo
    #[arg(long, value_name = "UTENTE")]
    reset_user: Option<String>,
    /// Cartella con la build del pannello di onboarding (servita sulla porta admin)
    #[arg(long, env = "OTR_UI_DIR", default_value = "./web/dist")]
    ui_dir: PathBuf,
    /// Cartella con la guida costruita: se esiste è servita sulla porta admin sotto /docs/
    #[arg(long, env = "OTR_DOCS_DIR")]
    docs_dir: Option<PathBuf>,
    /// Indirizzo della guida online per i link "Guida" del pannello (vuoto = nessun link)
    #[arg(
        long,
        env = "OTR_DOCS_URL",
        default_value = "https://garzuu.github.io/OtterRoute/"
    )]
    docs_url: String,
    /// Indirizzo HTTPS pubblico (vuoto = HTTPS disattivato). Serve un certificato per dominio:
    /// si ottengono in automatico dal pannello (Impostazioni → HTTPS).
    #[arg(long, env = "OTR_HTTPS_LISTEN", default_value = "0.0.0.0:443")]
    https_listen: String,
    /// Directory ACME alternativa a Let's Encrypt (per le prove, es. Pebble)
    #[arg(long, env = "OTR_ACME_DIRECTORY")]
    acme_directory: Option<String>,
    /// Certificato radice della CA della directory ACME alternativa
    #[arg(long, env = "OTR_ACME_CA_ROOT")]
    acme_ca_root: Option<PathBuf>,
    /// Indirizzo con cui si raggiunge il pannello, per il link nelle notifiche (facoltativo)
    #[arg(long, env = "OTR_PUBLIC_URL")]
    public_url: Option<String>,
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

    // nel processo convivono due provider crittografici di rustls (ring e aws-lc-rs):
    // senza un predefinito esplicito i client TLS (ACME, SMTP, reqwest) vanno in panic
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args = Args::parse();
    std::fs::create_dir_all(&args.state_dir).context("state_dir")?;
    if let Some(name) = &args.reset_user {
        return reset_user(&args.state_dir, name);
    }
    audit::init(&args.state_dir);
    let last_good = args.state_dir.join("last-good.yaml");

    let snapshot = initial_snapshot(&args.config, &last_good)?;
    if snapshot.version == 0 {
        tracing::warn!(
            "nessuna configurazione: apri il pannello sulla porta admin per il primo accesso"
        );
    }
    tracing::info!(
        version = snapshot.version,
        routes = snapshot.route_count(),
        "configurazione attiva"
    );

    // HTTPS: certificati salvati, domini con redirect e porta pubblica dal pannello
    let panel_now = panel::load(&args.state_dir).unwrap_or_default();
    let tls_state = tls::Tls::new(panel_now.settings.https());
    let loaded = tls::load_all(&args.state_dir, &tls_state.store);
    tls_state.apply_panel(&panel_now);
    if loaded > 0 {
        tracing::info!(certificati = loaded, "certificati HTTPS caricati");
    }
    let acme = acme::Acme::new(
        args.state_dir.clone(),
        tls_state.clone(),
        args.acme_directory.clone(),
        args.acme_ca_root.clone(),
    );

    let cache = Cache::open(&args.cache_dir, args.cache_max_bytes).context("apertura cache")?;
    let metrics_file = args.state_dir.join("metrics.json");
    let metrics = metrics::Metrics::load(&metrics_file);
    let state = Arc::new(AppState {
        snapshot: ArcSwap::from_pointee(snapshot),
        cache,
        max_object_bytes: args.cache_max_object_bytes,
        node_id: load_node_id(&args.state_dir),
        metrics: metrics.clone(),
        signing_key: ArcSwap::from_pointee(sign::load_or_create(&args.state_dir)),
        tls: tls_state.clone(),
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
    let notifier = Arc::new(notify::Notifier::new(
        args.state_dir.clone(),
        state.node_id.clone(),
        args.public_url.clone().filter(|u| !u.is_empty()),
    ));
    let admin_ctx = Arc::new(admin::Admin {
        state: state.clone(),
        config_path: args.config.clone(),
        state_dir: args.state_dir.clone(),
        ui_dir: args.ui_dir.clone(),
        docs_dir: args.docs_dir.clone(),
        docs_url: args.docs_url.clone(),
        listen_port: args.listen.port(),
        recheck_pending: args.domain_recheck,
        recheck_verified: args.domain_recheck_verified,
        write_lock: tokio::sync::Mutex::new(()),
        auth: auth::Auth::new(&args.state_dir),
        notifier: notifier.clone(),
        acme: acme.clone(),
    });
    tokio::spawn(acme.clone().run());
    tokio::spawn(notify::run(notifier));
    tokio::spawn(admin::recheck_loop(admin_ctx.clone()));
    tokio::spawn(metrics::flush_loop(metrics.clone(), metrics_file.clone()));
    tokio::spawn(serve(admin, move |req| {
        admin::handle(admin_ctx.clone(), req)
    }));

    let public = TcpListener::bind(args.listen)
        .await
        .context("bind pubblico")?;
    tracing::info!(addr = %args.listen, "gateway in ascolto");
    let st = state.clone();
    let server = serve(public, move |req| handler::handle(st.clone(), req, false));

    // HTTPS: un errore di bind non ferma il nodo (la porta 443 può servire privilegi)
    if !args.https_listen.trim().is_empty() {
        match (
            args.https_listen.parse::<SocketAddr>(),
            tls::acceptor(tls_state.store.clone()),
        ) {
            (Ok(addr), Ok(acceptor)) => match TcpListener::bind(addr).await {
                Ok(l) => {
                    tracing::info!(addr = %addr, "HTTPS in ascolto");
                    tls_state.set_listening(true);
                    let st = state.clone();
                    tokio::spawn(tls::serve(l, acceptor, move |req| {
                        handler::handle(st.clone(), req, true)
                    }));
                }
                Err(e) => {
                    tracing::warn!(addr = %addr, error = %e, "HTTPS non attivo: porta non disponibile")
                }
            },
            (Err(e), _) => {
                tracing::warn!(error = %e, "OTR_HTTPS_LISTEN non valido: HTTPS non attivo")
            }
            (_, Err(e)) => {
                tracing::warn!(error = %e, "configurazione TLS non valida: HTTPS non attivo")
            }
        }
    }

    tokio::select! {
        _ = server => {}
        _ = shutdown_signal() => tracing::info!("arresto"),
    }
    metrics.save(&metrics_file);
    Ok(())
}

/// Configurazione all'avvio: il file indicato, oppure l'ultima valida salvata.
/// Così un nodo che riparte mentre il pannello è giù continua a servire.
/// Se non esiste nulla (primo avvio) parte vuoto: la crea l'onboarding.
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
            if !config.exists() && !last_good.exists() {
                return Ok(config::Snapshot::empty());
            }
            tracing::error!(error = %e, "configurazione non utilizzabile, provo l'ultima valida");
            let raw =
                std::fs::read(last_good).context("nessuna configurazione valida disponibile")?;
            config::parse(&raw).context("anche l'ultima configurazione valida è inutilizzabile")
        }
    }
}

/// Recupero d'emergenza: nuova password temporanea, 2FA disattivata, utente
/// riabilitato e sbloccato. Si lancia dal terminale del nodo, quindi presuppone
/// l'accesso alla macchina.
fn reset_user(state_dir: &Path, name: &str) -> anyhow::Result<()> {
    let store = users::Store::load(state_dir);
    let user = store
        .find_by_name(name)
        .with_context(|| format!("utente '{name}' non trovato"))?;
    let temp = users::temp_password();
    let hash = users::hash_password(&temp).map_err(anyhow::Error::msg)?;
    store
        .update(&user.id, |u| {
            u.password_hash = hash;
            u.must_change_password = true;
            u.disabled = false;
            u.totp = Default::default();
            u.recovery_hashes.clear();
            Ok(())
        })
        .map_err(anyhow::Error::msg)?;
    println!("Password temporanea per '{}': {temp}", user.username);
    println!("Al primo accesso verrà chiesto di sceglierne una nuova; la 2FA è stata disattivata.");
    Ok(())
}

/// Identità stabile del nodo (creata al primo avvio).
fn load_node_id(state_dir: &Path) -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let file = state_dir.join("node-id");
    if let Ok(id) = std::fs::read_to_string(&file) {
        if !id.trim().is_empty() {
            return id.trim().to_owned();
        }
    }
    let mut raw = [0u8; 16];
    OsRng.fill_bytes(&mut raw);
    let id = hex::encode(raw);
    if let Err(e) = std::fs::write(&file, &id) {
        tracing::warn!(error = %e, "impossibile salvare l'identità del nodo");
    }
    id
}

pub(crate) fn save_last_good(path: &Path, raw: &[u8]) {
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
                if snap.version == current {
                    // già applicata direttamente dal pannello
                    tracing::debug!(version = current, "configurazione già attiva");
                    continue;
                }
                if snap.version < current {
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
