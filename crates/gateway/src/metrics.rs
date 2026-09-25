//! Statistiche di traffico del nodo: richieste, esito della cache, byte,
//! errori e latenza, per instradamento e per file. Servono alla dashboard del
//! pannello (`/api/metrics`) e a Prometheus (`/metrics`).
//!
//! Tutto vive in memoria e viene salvato su disco ogni minuto, così lo storico
//! sopravvive ai riavvii. Le serie temporali sono indicizzate sul tempo assoluto
//! (minuto / ora UNIX) e potate man mano: nessun anello da riallineare.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::body::Body;
use crate::cache::Stats as CacheStats;

/// Slot al minuto conservati (per la vista "1 h" servono 60; teniamo un margine).
const MIN_SLOTS: u64 = 120;
/// Slot all'ora conservati: 7 giorni.
const HOUR_SLOTS: u64 = 168;
/// Voci massime della classifica dei file.
const TOP_MAX: usize = 5000;
/// Fasce dell'istogramma della latenza, in millisecondi (l'ultima è "oltre").
pub const LAT_BOUNDS_MS: [u64; 10] = [5, 10, 25, 50, 100, 250, 500, 1000, 2500, u64::MAX];

// ---------------------------------------------------------------------------
// Attribuzione di una richiesta a instradamento e storage
// ---------------------------------------------------------------------------

/// Dati raccolti mentre la richiesta viene servita; li riempie il gestore.
#[derive(Default)]
pub struct ReqTag {
    route: Mutex<Option<String>>,
    upstream_storage: Mutex<Option<String>>,
}

tokio::task_local! {
    static REQ: Arc<ReqTag>;
}

/// Esegue `f` con `tag` come richiesta corrente.
pub async fn scope<F: Future>(tag: Arc<ReqTag>, f: F) -> F::Output {
    REQ.scope(tag, f).await
}

/// Il gestore ha risolto la regola: da qui in poi la richiesta è "di" quella regola.
pub fn note_route(id: &str) {
    let _ = REQ.try_with(|t| *t.route.lock().unwrap() = Some(id.to_owned()));
}

/// Lo storage non ha risposto o ha rifiutato la richiesta.
pub fn note_upstream_error(storage: &str) {
    let _ = REQ.try_with(|t| *t.upstream_storage.lock().unwrap() = Some(storage.to_owned()));
}

impl ReqTag {
    fn route(&self) -> Option<String> {
        self.route.lock().unwrap().clone()
    }
    fn upstream(&self) -> Option<String> {
        self.upstream_storage.lock().unwrap().clone()
    }
}

/// Una richiesta conclusa.
pub struct RequestSample {
    pub route: Option<String>,
    pub path: String,
    pub status: u16,
    /// valore di `X-Cache` (HIT, MISS, …) se c'era
    pub cache: Option<String>,
    pub latency_ms: u64,
    pub bytes: u64,
    pub upstream_storage: Option<String>,
}

impl RequestSample {
    pub fn new(
        tag: &ReqTag,
        path: &str,
        status: u16,
        cache: Option<&str>,
        latency_ms: u64,
    ) -> Self {
        RequestSample {
            route: tag.route(),
            path: path.chars().take(200).collect(),
            status,
            cache: cache.map(str::to_owned),
            latency_ms,
            bytes: 0,
            upstream_storage: tag.upstream(),
        }
    }
}

// ---------------------------------------------------------------------------
// Contatori
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Serialize, Deserialize)]
struct Counts {
    req: u64,
    hit: u64,
    miss: u64,
    stale: u64,
    reval: u64,
    bypass: u64,
    s2: u64,
    s3: u64,
    s4: u64,
    s5: u64,
    bytes: u64,
    upstream: u64,
    lat: [u64; 10],
}

impl Counts {
    fn add_sample(&mut self, s: &RequestSample) {
        self.req += 1;
        match s.cache.as_deref() {
            Some("HIT") => self.hit += 1,
            Some("MISS") => self.miss += 1,
            Some("STALE") => self.stale += 1,
            Some("REVALIDATED") => self.reval += 1,
            Some("BYPASS") => self.bypass += 1,
            _ => {}
        }
        match s.status {
            200..=299 => self.s2 += 1,
            300..=399 => self.s3 += 1,
            400..=499 => self.s4 += 1,
            500..=599 => self.s5 += 1,
            _ => {}
        }
        self.bytes += s.bytes;
        if s.upstream_storage.is_some() {
            self.upstream += 1;
        }
        self.lat[lat_bucket(s.latency_ms)] += 1;
    }

    fn merge(&mut self, o: &Counts) {
        self.req += o.req;
        self.hit += o.hit;
        self.miss += o.miss;
        self.stale += o.stale;
        self.reval += o.reval;
        self.bypass += o.bypass;
        self.s2 += o.s2;
        self.s3 += o.s3;
        self.s4 += o.s4;
        self.s5 += o.s5;
        self.bytes += o.bytes;
        self.upstream += o.upstream;
        for (a, b) in self.lat.iter_mut().zip(o.lat.iter()) {
            *a += b;
        }
    }

    /// Quota servita dalla cache tra le risposte che hanno un esito di cache
    /// (i 404 e gli errori non ne hanno e non contano).
    fn hit_ratio(&self) -> Option<f64> {
        let served = self.hit + self.stale + self.reval;
        let total = served + self.miss + self.bypass;
        (total > 0).then(|| served as f64 / total as f64)
    }
}

fn lat_bucket(ms: u64) -> usize {
    LAT_BOUNDS_MS
        .iter()
        .position(|&b| ms <= b)
        .unwrap_or(LAT_BOUNDS_MS.len() - 1)
}

/// Stima del percentile dal limite superiore della fascia che lo contiene.
fn percentile(lat: &[u64; 10], q: f64) -> Option<u64> {
    let total: u64 = lat.iter().sum();
    if total == 0 {
        return None;
    }
    let target = (total as f64 * q).ceil().max(1.0) as u64;
    let mut acc = 0;
    for (i, n) in lat.iter().enumerate() {
        acc += n;
        if acc >= target {
            return Some(LAT_BOUNDS_MS[i]);
        }
    }
    None
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Series {
    minutes: BTreeMap<u64, Counts>,
    hours: BTreeMap<u64, Counts>,
}

impl Series {
    fn record(&mut self, s: &RequestSample, now: u64) {
        let (m, h) = (now / 60, now / 3600);
        self.minutes.entry(m).or_default().add_sample(s);
        self.hours.entry(h).or_default().add_sample(s);
        self.prune(now);
    }

    fn prune(&mut self, now: u64) {
        let (m, h) = (now / 60, now / 3600);
        self.minutes = self.minutes.split_off(&m.saturating_sub(MIN_SLOTS - 1));
        self.hours = self.hours.split_off(&h.saturating_sub(HOUR_SLOTS - 1));
    }
}

/// Totali dall'inizio della raccolta, per Prometheus.
#[derive(Default, Clone, Serialize, Deserialize)]
struct RouteCum {
    /// "esito_cache|classe" → richieste
    requests: BTreeMap<String, u64>,
    bytes: u64,
    /// storage → errori
    upstream: BTreeMap<String, u64>,
    lat: [u64; 10],
    lat_sum_ms: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Inner {
    global: Series,
    routes: BTreeMap<String, Series>,
    cum: BTreeMap<String, RouteCum>,
    codes: BTreeMap<u16, u64>,
    /// "instradamento\0percorso" → (richieste, byte)
    top: HashMap<String, (u64, u64)>,
    #[serde(skip)]
    dirty: bool,
}

pub const UNROUTED: &str = "-";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Range {
    Hour,
    Day,
    Week,
}

impl Range {
    pub fn parse(s: &str) -> Option<Range> {
        match s {
            "1h" => Some(Range::Hour),
            "24h" => Some(Range::Day),
            "7d" => Some(Range::Week),
            _ => None,
        }
    }
}

pub struct Metrics {
    inner: Mutex<Inner>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Metrics {
    #[cfg(test)]
    pub fn new() -> Arc<Self> {
        Arc::new(Metrics {
            inner: Mutex::new(Inner::default()),
        })
    }

    /// Carica lo storico salvato; se manca o è illeggibile si riparte da zero.
    pub fn load(path: &Path) -> Arc<Self> {
        let mut inner: Inner = std::fs::read(path)
            .ok()
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or_default();
        let now = now_secs();
        inner.global.prune(now);
        for s in inner.routes.values_mut() {
            s.prune(now);
        }
        Arc::new(Metrics {
            inner: Mutex::new(inner),
        })
    }

    /// Salva lo stato (in modo atomico) se è cambiato dall'ultimo salvataggio.
    pub fn save(&self, path: &Path) {
        let data = {
            let mut i = self.inner.lock().unwrap();
            if !i.dirty {
                return;
            }
            i.dirty = false;
            serde_json::to_vec(&*i)
        };
        match data {
            Ok(d) => {
                if let Err(e) = crate::admin::write_atomic(path, &d, false) {
                    tracing::warn!(error = %e, "impossibile salvare le statistiche");
                    self.inner.lock().unwrap().dirty = true;
                }
            }
            Err(e) => tracing::warn!(error = %e, "impossibile serializzare le statistiche"),
        }
    }

    pub fn record(&self, s: RequestSample) {
        self.record_at(s, now_secs());
    }

    fn record_at(&self, s: RequestSample, now: u64) {
        let route = s.route.clone().unwrap_or_else(|| UNROUTED.to_owned());
        let mut i = self.inner.lock().unwrap();
        i.dirty = true;
        i.global.record(&s, now);
        i.routes.entry(route.clone()).or_default().record(&s, now);
        *i.codes.entry(s.status).or_default() += 1;

        let cum = i.cum.entry(route.clone()).or_default();
        let class = match s.status {
            200..=299 => "2xx",
            300..=399 => "3xx",
            400..=499 => "4xx",
            _ => "5xx",
        };
        let cache = s.cache.as_deref().unwrap_or("none");
        *cum.requests.entry(format!("{cache}|{class}")).or_default() += 1;
        cum.bytes += s.bytes;
        cum.lat[lat_bucket(s.latency_ms)] += 1;
        cum.lat_sum_ms += s.latency_ms;
        if let Some(st) = &s.upstream_storage {
            *cum.upstream.entry(st.clone()).or_default() += 1;
        }

        if s.route.is_some() && s.status < 400 {
            if i.top.len() >= TOP_MAX && !i.top.contains_key(&top_key(&route, &s.path)) {
                trim_top(&mut i.top);
            }
            let e = i.top.entry(top_key(&route, &s.path)).or_default();
            e.0 += 1;
            e.1 += s.bytes;
        }
    }

    /// Avvolge il corpo per contare i byte davvero inviati; la richiesta viene
    /// registrata quando il corpo finisce o viene abbandonato dal client.
    pub fn track(self: &Arc<Self>, body: Body, sample: RequestSample) -> Body {
        let mut guard = Guard {
            metrics: self.clone(),
            sample: Some(sample),
            bytes: 0,
        };
        body.map_frame(move |f| {
            // riferimento all'intero `guard`: così la chiusura ne è proprietaria e
            // la richiesta si registra solo quando il corpo viene lasciato
            let g = &mut guard;
            if let Some(d) = f.data_ref() {
                g.bytes += d.len() as u64;
            }
            f
        })
        .boxed_unsync()
    }

    pub fn snapshot(&self, range: Range) -> Value {
        self.snapshot_at(range, now_secs())
    }

    fn snapshot_at(&self, range: Range, now: u64) -> Value {
        let i = self.inner.lock().unwrap();
        let (step, points) = match range {
            Range::Hour => (60, 60u64),
            Range::Day => (3600, 24),
            Range::Week => (3600, 168),
        };
        let last = now / step;
        let slots: Vec<u64> = (last + 1 - points..=last).collect();
        let pick = |s: &Series| -> Vec<Counts> {
            let map = if range == Range::Hour {
                &s.minutes
            } else {
                &s.hours
            };
            slots
                .iter()
                .map(|t| map.get(t).cloned().unwrap_or_default())
                .collect()
        };
        let sum = |v: &[Counts]| {
            let mut t = Counts::default();
            v.iter().for_each(|c| t.merge(c));
            t
        };

        let g = pick(&i.global);
        let total = sum(&g);
        let series: Vec<Value> = slots
            .iter()
            .zip(&g)
            .map(|(t, c)| {
                json!({
                    "t": t * step,
                    "req": c.req,
                    "hit": c.hit,
                    "miss": c.miss,
                    "other": c.req.saturating_sub(c.hit + c.miss),
                    "bytes": c.bytes,
                    "err": c.s5,
                })
            })
            .collect();

        let mut routes: Vec<Value> = i
            .routes
            .iter()
            .filter_map(|(id, s)| {
                let c = sum(&pick(s));
                (c.req > 0).then(|| {
                    json!({
                        "id": id,
                        "requests": c.req,
                        "hit_ratio": c.hit_ratio(),
                        "bytes": c.bytes,
                        "errors": c.s5,
                        "upstream_errors": c.upstream,
                        "p95": percentile(&c.lat, 0.95),
                    })
                })
            })
            .collect();
        routes.sort_by(|a, b| b["requests"].as_u64().cmp(&a["requests"].as_u64()));

        let mut top: Vec<(&String, &(u64, u64))> = i.top.iter().collect();
        top.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
        let top_files: Vec<Value> = top
            .into_iter()
            .take(20)
            .map(|(k, (n, b))| {
                let (route, path) = k.split_once('\0').unwrap_or((k, ""));
                json!({"route": route, "path": path, "requests": n, "bytes": b})
            })
            .collect();

        let lat: Vec<Value> = LAT_BOUNDS_MS
            .iter()
            .zip(total.lat.iter())
            .map(|(b, n)| json!({"le": if *b == u64::MAX { Value::Null } else { json!(b) }, "count": n}))
            .collect();

        json!({
            "range": match range { Range::Hour => "1h", Range::Day => "24h", Range::Week => "7d" },
            "totals": {
                "requests": total.req,
                "hit_ratio": total.hit_ratio(),
                "bytes": total.bytes,
                "errors_5xx": total.s5,
                "upstream_errors": total.upstream,
                "latency": {
                    "p50": percentile(&total.lat, 0.50),
                    "p95": percentile(&total.lat, 0.95),
                    "p99": percentile(&total.lat, 0.99),
                },
            },
            "classes": {"2xx": total.s2, "3xx": total.s3, "4xx": total.s4, "5xx": total.s5},
            "latency_histogram": lat,
            "series": series,
            "routes": routes,
            "top_files": top_files,
        })
    }

    /// Formato testuale di Prometheus.
    pub fn render_prometheus(&self, cache: &CacheStats, config_version: u64) -> String {
        let i = self.inner.lock().unwrap();
        let esc = |s: &str| {
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        };
        let mut o = String::new();
        let family = |o: &mut String, name: &str, help: &str, kind: &str| {
            o.push_str(&format!("# HELP {name} {help}\n# TYPE {name} {kind}\n"));
        };

        family(
            &mut o,
            "otterroute_requests_total",
            "Richieste servite",
            "counter",
        );
        for (route, c) in &i.cum {
            for (k, n) in &c.requests {
                let (cache_out, class) = k.split_once('|').unwrap_or((k, ""));
                o.push_str(&format!(
                    "otterroute_requests_total{{route=\"{}\",cache=\"{}\",class=\"{}\"}} {n}\n",
                    esc(route),
                    esc(cache_out),
                    class
                ));
            }
        }
        family(
            &mut o,
            "otterroute_response_bytes_total",
            "Byte inviati ai client",
            "counter",
        );
        for (route, c) in &i.cum {
            o.push_str(&format!(
                "otterroute_response_bytes_total{{route=\"{}\"}} {}\n",
                esc(route),
                c.bytes
            ));
        }
        family(
            &mut o,
            "otterroute_upstream_errors_total",
            "Errori dello storage",
            "counter",
        );
        for (route, c) in &i.cum {
            for (st, n) in &c.upstream {
                o.push_str(&format!(
                    "otterroute_upstream_errors_total{{route=\"{}\",storage=\"{}\"}} {n}\n",
                    esc(route),
                    esc(st)
                ));
            }
        }
        family(
            &mut o,
            "otterroute_request_duration_seconds",
            "Latenza fino alla risposta",
            "histogram",
        );
        for (route, c) in &i.cum {
            let r = esc(route);
            let mut acc = 0;
            for (b, n) in LAT_BOUNDS_MS.iter().zip(c.lat.iter()) {
                acc += n;
                let le = if *b == u64::MAX {
                    "+Inf".to_owned()
                } else {
                    format!("{}", *b as f64 / 1000.0)
                };
                o.push_str(&format!("otterroute_request_duration_seconds_bucket{{route=\"{r}\",le=\"{le}\"}} {acc}\n"));
            }
            o.push_str(&format!(
                "otterroute_request_duration_seconds_sum{{route=\"{r}\"}} {}\n",
                c.lat_sum_ms as f64 / 1000.0
            ));
            o.push_str(&format!(
                "otterroute_request_duration_seconds_count{{route=\"{r}\"}} {acc}\n"
            ));
        }
        for (name, help, v) in [
            (
                "otterroute_cache_bytes",
                "Byte occupati dalla cache",
                cache.bytes as f64,
            ),
            (
                "otterroute_cache_max_bytes",
                "Spazio massimo della cache",
                cache.max_bytes as f64,
            ),
            (
                "otterroute_cache_entries",
                "Oggetti in cache",
                cache.entries as f64,
            ),
            (
                "otterroute_cache_inflight",
                "Download in corso verso la cache",
                cache.inflight as f64,
            ),
            (
                "otterroute_config_version",
                "Versione della configurazione attiva",
                config_version as f64,
            ),
        ] {
            family(&mut o, name, help, "gauge");
            o.push_str(&format!("{name} {v}\n"));
        }
        o
    }
}

fn top_key(route: &str, path: &str) -> String {
    format!("{route}\0{path}")
}

/// A saturazione scarta il 10 % delle voci meno richieste.
fn trim_top(top: &mut HashMap<String, (u64, u64)>) {
    let mut counts: Vec<u64> = top.values().map(|v| v.0).collect();
    counts.sort_unstable();
    let cut = counts[counts.len() / 10];
    top.retain(|_, v| v.0 > cut);
}

struct Guard {
    metrics: Arc<Metrics>,
    sample: Option<RequestSample>,
    bytes: u64,
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(mut s) = self.sample.take() {
            s.bytes = self.bytes;
            self.metrics.record(s);
        }
    }
}

/// Salvataggio periodico dello storico.
pub async fn flush_loop(metrics: Arc<Metrics>, path: PathBuf) {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop {
        tick.tick().await;
        metrics.save(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(
        route: &str,
        path: &str,
        status: u16,
        cache: Option<&str>,
        ms: u64,
        bytes: u64,
    ) -> RequestSample {
        RequestSample {
            route: Some(route.into()),
            path: path.into(),
            status,
            cache: cache.map(Into::into),
            latency_ms: ms,
            bytes,
            upstream_storage: None,
        }
    }

    #[test]
    fn totals_and_hit_ratio() {
        let m = Metrics::new();
        let t = 1_800_000_000;
        m.record_at(sample("r", "/a", 200, Some("MISS"), 30, 100), t);
        m.record_at(sample("r", "/a", 200, Some("HIT"), 2, 100), t + 1);
        m.record_at(sample("r", "/a", 200, Some("HIT"), 2, 100), t + 2);
        m.record_at(sample("r", "/x", 404, None, 8, 10), t + 3);
        let s = m.snapshot_at(Range::Hour, t + 10);
        assert_eq!(s["totals"]["requests"], 4);
        assert_eq!(s["totals"]["bytes"], 310);
        // 2 dalla cache su 3 risposte con esito (il 404 non conta)
        let ratio = s["totals"]["hit_ratio"].as_f64().unwrap();
        assert!((ratio - 2.0 / 3.0).abs() < 1e-9, "{ratio}");
        assert_eq!(s["classes"]["4xx"], 1);
        assert_eq!(s["routes"][0]["requests"], 4);
        assert_eq!(s["series"].as_array().unwrap().len(), 60);
    }

    #[tokio::test]
    async fn track_counts_bytes_when_body_is_consumed() {
        let m = Metrics::new();
        let tracked = m.track(
            crate::body::full("hello world"),
            sample("r", "/a", 200, Some("MISS"), 1, 0),
        );
        // finché il corpo esiste la richiesta non è ancora registrata
        assert_eq!(m.snapshot(Range::Hour)["totals"]["requests"], 0);
        let bytes = tracked.collect().await.unwrap().to_bytes();
        assert_eq!(bytes.len(), 11);
        let s = m.snapshot(Range::Hour);
        assert_eq!(s["totals"]["requests"], 1);
        assert_eq!(s["totals"]["bytes"], 11);
    }

    #[test]
    fn old_slots_expire() {
        let m = Metrics::new();
        let t = 1_800_000_000;
        m.record_at(sample("r", "/a", 200, Some("HIT"), 1, 1), t);
        // otto giorni dopo lo slot è fuori da ogni finestra
        m.record_at(sample("r", "/b", 200, Some("HIT"), 1, 1), t + 8 * 86400);
        let s = m.snapshot_at(Range::Week, t + 8 * 86400);
        assert_eq!(s["totals"]["requests"], 1);
        // e la vista a un'ora vede solo l'ultima
        let h = m.snapshot_at(Range::Hour, t + 8 * 86400);
        assert_eq!(h["totals"]["requests"], 1);
    }

    #[test]
    fn percentiles_use_bucket_bounds() {
        let mut lat = [0u64; 10];
        lat[0] = 90; // ≤ 5 ms
        lat[4] = 9; // ≤ 100 ms
        lat[9] = 1; // oltre
        assert_eq!(percentile(&lat, 0.50), Some(5));
        assert_eq!(percentile(&lat, 0.95), Some(100));
        assert_eq!(percentile(&lat, 1.0), Some(u64::MAX));
        assert_eq!(percentile(&[0; 10], 0.5), None);
    }

    #[test]
    fn top_files_are_bounded() {
        let m = Metrics::new();
        for n in 0..(TOP_MAX + 200) {
            m.record_at(
                sample("r", &format!("/f{n}"), 200, Some("HIT"), 1, 1),
                1_800_000_000,
            );
        }
        // un file molto richiesto sopravvive alla potatura
        for _ in 0..50 {
            m.record_at(
                sample("r", "/popolare", 200, Some("HIT"), 1, 1),
                1_800_000_000,
            );
        }
        let i = m.inner.lock().unwrap();
        assert!(i.top.len() <= TOP_MAX + 1, "{}", i.top.len());
        assert!(i.top.contains_key(&top_key("r", "/popolare")));
    }

    #[test]
    fn persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("metrics.json");
        let m = Metrics::new();
        m.record(sample("r", "/a", 200, Some("HIT"), 3, 42));
        m.save(&file);
        let loaded = Metrics::load(&file);
        let s = loaded.snapshot(Range::Hour);
        assert_eq!(s["totals"]["requests"], 1);
        assert_eq!(s["totals"]["bytes"], 42);
        // senza modifiche non riscrive
        std::fs::remove_file(&file).unwrap();
        loaded.save(&file);
        assert!(!file.exists());
    }

    #[test]
    fn prometheus_output_has_counters() {
        let m = Metrics::new();
        m.record(sample("r1", "/a", 200, Some("HIT"), 3, 42));
        let out = m.render_prometheus(
            &CacheStats {
                entries: 1,
                bytes: 10,
                max_bytes: 100,
                inflight: 0,
            },
            7,
        );
        assert!(
            out.contains("otterroute_requests_total{route=\"r1\",cache=\"HIT\",class=\"2xx\"} 1")
        );
        assert!(out.contains("otterroute_response_bytes_total{route=\"r1\"} 42"));
        assert!(out.contains("otterroute_request_duration_seconds_count{route=\"r1\"} 1"));
        assert!(out.contains("otterroute_config_version 7"));
    }
}
