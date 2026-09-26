import { useEffect, useState } from "react";
import { api, type MetricsRange, type MetricsResponse, type PanelState } from "../api";
import { HBars, Histogram, Meter, RequestsChart, fmtMs, fmtNum, fmtPct } from "../charts";
import { DataTable, type Column } from "../DataTable";
import { loc } from "../i18n";
import { useAuth } from "../auth";
import { canSetup, setupDone, setupSteps } from "../setup";
import { EmptyState, Illus, Page, fmtBytes } from "../ui";

const RANGES: { id: MetricsRange; label: string }[] = [
  { id: "1h", label: "1 ora" },
  { id: "24h", label: "24 ore" },
  { id: "7d", label: "7 giorni" },
];

type RouteRow = MetricsResponse["routes"][number];
type FileRow = MetricsResponse["top_files"][number];

const routeColumns: Column<RouteRow>[] = [
  {
    key: "route",
    header: "Instradamento",
    sort: (r) => r.match ?? r.id,
    render: (r) => <code className="big">{r.match ?? (r.id === "-" ? "non instradato" : r.id)}</code>,
  },
  { key: "req", header: "Richieste", align: "right", sort: (r) => r.requests, render: (r) => fmtNum(r.requests) },
  { key: "hit", header: "Cache hit", align: "right", sort: (r) => r.hit_ratio ?? -1, render: (r) => fmtPct(r.hit_ratio) },
  { key: "bytes", header: "Banda", align: "right", sort: (r) => r.bytes, render: (r) => fmtBytes(r.bytes) },
  {
    key: "err",
    header: "Errori",
    align: "right",
    sort: (r) => r.errors,
    render: (r) => (r.errors ? <span className="bad-text">{fmtNum(r.errors)}</span> : "0"),
  },
  { key: "p95", header: "p95", align: "right", sort: (r) => r.p95 ?? -1, render: (r) => fmtMs(r.p95) },
];

const fileColumns: Column<FileRow>[] = [
  {
    key: "file",
    header: "File",
    sort: (f) => f.path,
    render: (f) => (
      <code>
        {(f.match ?? "").split("/")[0]}
        {f.path}
      </code>
    ),
  },
  { key: "req", header: "Richieste", align: "right", sort: (f) => f.requests, render: (f) => fmtNum(f.requests) },
  { key: "bytes", header: "Banda", align: "right", sort: (f) => f.bytes, render: (f) => fmtBytes(f.bytes) },
];

function Kpi(props: { label: string; value: string; sub?: string; tone?: "bad" }) {
  return (
    <div className="stat">
      <span className="muted">{props.label}</span>
      <strong className={props.tone === "bad" ? "bad-text" : undefined}>{props.value}</strong>
      {props.sub && <span className="muted small-text">{props.sub}</span>}
    </div>
  );
}

export function Overview(props: { state: PanelState; onWizard: () => void }) {
  const { state } = props;
  const { can } = useAuth();
  const canMetrics = can("metrics:read");
  const { domains, buckets } = state.panel;
  const [range, setRange] = useState<MetricsRange>("24h");
  const [data, setData] = useState<MetricsResponse | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!canMetrics) return;
    let live = true;
    const load = () =>
      api
        .metrics(range)
        .then((m) => live && (setData(m), setErr(null)))
        .catch((e: Error) => live && setErr(e.message));
    void load();
    const t = setInterval(load, 15000);
    return () => {
      live = false;
      clearInterval(t);
    };
  }, [range, canMetrics]);

  const verified = domains.filter((d) => d.verified).length;
  const steps = setupSteps(state);
  const nextStep = steps.find((s) => !s.done);

  const t = data?.totals;
  const errRate = t && t.requests ? t.errors_5xx / t.requests : 0;
  const cache = data?.cache ?? state.cache;

  return (
    <Page title="Panoramica">
      {nextStep && canSetup(can) && (
        <div className="setup-banner" role="status">
          <Illus name="laptop" width={64} />
          <div className="grow">
            <strong>Completa la configurazione</strong>
            <div className="muted">
              Prossimo passo: {nextStep.title}
            </div>
            <div className="sprog" aria-label={`${setupDone(steps)} passi su ${steps.length}`}>
              {steps.map((s) => (
                <span key={s.id} className={s.done ? "on" : ""} title={s.title} />
              ))}
            </div>
          </div>
          <button className="primary" onClick={props.onWizard}>
            {setupDone(steps) === 0 ? "Inizia" : "Continua"}
          </button>
        </div>
      )}

      {canMetrics && (
        <>
      <div className="toolbar">
        <div className="seg" role="tablist" aria-label="Periodo">
          {RANGES.map((r) => (
            <button key={r.id} role="tab" aria-selected={r.id === range} className={r.id === range ? "on" : ""} onClick={() => setRange(r.id)}>
              {r.label}
            </button>
          ))}
        </div>
        <span className="muted small-text">Aggiornato ogni 15 secondi · le statistiche partono dall’avvio della raccolta</span>
      </div>
      {err && <div className="box bad">{err}</div>}

      <div className="stats five">
        <Kpi label="Richieste" value={t ? fmtNum(t.requests) : "…"} />
        <Kpi label="Cache hit" value={t ? fmtPct(t.hit_ratio) : "…"} sub="servite dalla cache" />
        <Kpi label="Banda servita" value={t ? fmtBytes(t.bytes) : "…"} />
        <Kpi
          label="Errori"
          value={t ? fmtPct(errRate) : "…"}
          sub={t ? `${fmtNum(t.errors_5xx)} 5xx · ${fmtNum(t.upstream_errors)} storage` : undefined}
          tone={t && t.errors_5xx > 0 ? "bad" : undefined}
        />
        <Kpi label="Latenza p95" value={t ? fmtMs(t.latency.p95) : "…"} sub={t ? `p50 ${fmtMs(t.latency.p50)} · p99 ${fmtMs(t.latency.p99)}` : undefined} />
      </div>

      <div className="card">
        <h2>Richieste nel tempo</h2>
        {data && data.totals.requests === 0 ? (
          <EmptyState image="sleeping" title="Nessuna richiesta nel periodo" text="Appena un visitatore aprirà un file dai tuoi domini, compariranno qui." />
        ) : data ? (
          <RequestsChart series={data.series} range={range} />
        ) : (
          <div className="muted">Caricamento…</div>
        )}
      </div>

      <div className="grid3">
        <div className="card">
          <h2>Stati HTTP</h2>
          {data && (
            <HBars
              items={[
                { label: "2xx riuscite", value: data.classes["2xx"], tone: "good" },
                { label: "3xx (304…)", value: data.classes["3xx"], tone: "info" },
                { label: "4xx non trovato", value: data.classes["4xx"], tone: "warn" },
                { label: "5xx errori", value: data.classes["5xx"], tone: "bad" },
              ]}
            />
          )}
        </div>
        <div className="card">
          <h2>Latenza</h2>
          {data && (
            <Histogram
              bins={data.latency_histogram.map((b) => ({ label: b.le === null ? ">2,5s" : b.le >= 1000 ? `${(b.le / 1000).toLocaleString(loc())}s` : `${b.le}`, count: b.count }))}
            />
          )}
          <p className="muted small-text mt">Millisecondi fino alla risposta.</p>
        </div>
        <div className="card">
          <h2>Cache su disco</h2>
          <Meter value={cache.bytes} max={cache.max_bytes}>
            <strong>{fmtBytes(cache.bytes)}</strong> <span className="muted">di {fmtBytes(cache.max_bytes)}</span>
          </Meter>
          <dl className="kv">
            <dt>Oggetti</dt>
            <dd>{fmtNum(cache.entries)}</dd>
            <dt>Download in corso</dt>
            <dd>{fmtNum(cache.inflight)}</dd>
          </dl>
        </div>
      </div>

        </>
      )}

      <div className="stats">
        {can("domains:read") && <Kpi label="Domini verificati" value={`${verified}/${domains.length}`} />}
        {can("buckets:read") && <Kpi label="Bucket" value={String(buckets.length)} />}
        {can("routes:read") && <Kpi label="Instradamenti attivi" value={String(state.live_routes.length)} />}
        <Kpi label="Configurazione" value={`#${state.config_version}`} />
      </div>

      {canMetrics && (
        <>
      <div className="card">
        <h2>Per instradamento</h2>
        <DataTable
          columns={routeColumns}
          rows={data?.routes ?? []}
          rowKey={(r) => r.id}
          empty="Nessun traffico nel periodo."
          initialSort={{ key: "req", dir: "desc" }}
        />
      </div>

      <div className="card">
        <h2>File più richiesti</h2>
        <DataTable
          columns={fileColumns}
          rows={data?.top_files ?? []}
          rowKey={(f) => `${f.route}\0${f.path}`}
          empty="Ancora nessun file richiesto."
          initialSort={{ key: "req", dir: "desc" }}
        />
        <p className="muted small-text mt">Classifica dall’avvio della raccolta, non filtrata per periodo.</p>
      </div>

        </>
      )}

      <p className="muted foot">
        Nodo v{state.version} · configurazione #{state.config_version} · porte pubbliche {state.http_port} (HTTP) e {state.https_port} (HTTPS)
      </p>
    </Page>
  );
}
