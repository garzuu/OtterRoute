import { useEffect, useState } from "react";
import { api, type MetricsRange, type MetricsResponse, type PanelState } from "../api";
import { HBars, Histogram, Meter, RequestsChart, fmtMs, fmtNum, fmtPct } from "../charts";
import { DataTable, type Column } from "../DataTable";
import { loc, tr, type Key } from "../i18n";
import { useAuth } from "../auth";
import { canSetup, setupDone, setupSteps } from "../setup";
import { EmptyState, Illus, Page, fmtBytes } from "../ui";

const RANGES: { id: MetricsRange; label: Key }[] = [
  { id: "1h", label: "ov.range1h" },
  { id: "24h", label: "ov.range24h" },
  { id: "7d", label: "ov.range7d" },
];

type RouteRow = MetricsResponse["routes"][number];
type FileRow = MetricsResponse["top_files"][number];

const routeColumns = (): Column<RouteRow>[] => [
  {
    key: "route",
    header: tr("ov.colRoute"),
    sort: (r) => r.match ?? r.id,
    render: (r) => <code className="big">{r.match ?? (r.id === "-" ? tr("ov.notRouted") : r.id)}</code>,
  },
  { key: "req", header: tr("ov.colReq"), align: "right", sort: (r) => r.requests, render: (r) => fmtNum(r.requests) },
  { key: "hit", header: tr("ov.colHit"), align: "right", sort: (r) => r.hit_ratio ?? -1, render: (r) => fmtPct(r.hit_ratio) },
  { key: "bytes", header: tr("ov.colBw"), align: "right", sort: (r) => r.bytes, render: (r) => fmtBytes(r.bytes) },
  {
    key: "err",
    header: tr("ov.colErr"),
    align: "right",
    sort: (r) => r.errors,
    render: (r) => (r.errors ? <span className="bad-text">{fmtNum(r.errors)}</span> : "0"),
  },
  { key: "p95", header: "p95", align: "right", sort: (r) => r.p95 ?? -1, render: (r) => fmtMs(r.p95) },
];

const fileColumns = (): Column<FileRow>[] => [
  {
    key: "file",
    header: tr("ov.colFile"),
    sort: (f) => f.path,
    render: (f) => (
      <code>
        {(f.match ?? "").split("/")[0]}
        {f.path}
      </code>
    ),
  },
  { key: "req", header: tr("ov.colReq"), align: "right", sort: (f) => f.requests, render: (f) => fmtNum(f.requests) },
  { key: "bytes", header: tr("ov.colBw"), align: "right", sort: (f) => f.bytes, render: (f) => fmtBytes(f.bytes) },
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
    <Page title={tr("ov.title")}>
      {nextStep && canSetup(can) && (
        <div className="setup-banner" role="status">
          <Illus name="laptop" width={64} />
          <div className="grow">
            <strong>{tr("ov.finish")}</strong>
            <div className="muted">
              {tr("ov.nextStep", { title: nextStep.title })}
            </div>
            <div className="sprog" aria-label={tr("ov.stepsAria", { done: setupDone(steps), total: steps.length })}>
              {steps.map((s) => (
                <span key={s.id} className={s.done ? "on" : ""} title={s.title} />
              ))}
            </div>
          </div>
          <button className="primary" onClick={props.onWizard}>
            {setupDone(steps) === 0 ? tr("ov.start") : tr("common.continue")}
          </button>
        </div>
      )}

      {canMetrics && (
        <>
      <div className="toolbar">
        <div className="seg" role="tablist" aria-label={tr("ov.period")}>
          {RANGES.map((r) => (
            <button key={r.id} role="tab" aria-selected={r.id === range} className={r.id === range ? "on" : ""} onClick={() => setRange(r.id)}>
              {tr(r.label)}
            </button>
          ))}
        </div>
        <span className="muted small-text">{tr("ov.refresh")}</span>
      </div>
      {err && <div className="box bad">{err}</div>}

      <div className="stats five">
        <Kpi label={tr("ov.kReq")} value={t ? fmtNum(t.requests) : "…"} />
        <Kpi label={tr("ov.kHit")} value={t ? fmtPct(t.hit_ratio) : "…"} sub={tr("ov.kHitSub")} />
        <Kpi label={tr("ov.kBw")} value={t ? fmtBytes(t.bytes) : "…"} />
        <Kpi
          label={tr("ov.kErr")}
          value={t ? fmtPct(errRate) : "…"}
          sub={t ? tr("ov.kErrSub", { n5: fmtNum(t.errors_5xx), nu: fmtNum(t.upstream_errors) }) : undefined}
          tone={t && t.errors_5xx > 0 ? "bad" : undefined}
        />
        <Kpi label={tr("ov.kLat")} value={t ? fmtMs(t.latency.p95) : "…"} sub={t ? `p50 ${fmtMs(t.latency.p50)} · p99 ${fmtMs(t.latency.p99)}` : undefined} />
      </div>

      <div className="card">
        <h2>{tr("ov.chart")}</h2>
        {data && data.totals.requests === 0 ? (
          <EmptyState image="sleeping" title={tr("ov.noReq")} text={tr("ov.noReqText")} />
        ) : data ? (
          <RequestsChart series={data.series} range={range} />
        ) : (
          <div className="muted">{tr("ov.loading")}</div>
        )}
      </div>

      <div className="grid3">
        <div className="card">
          <h2>{tr("ov.http")}</h2>
          {data && (
            <HBars
              items={[
                { label: tr("ov.s2"), value: data.classes["2xx"], tone: "good" },
                { label: tr("ov.s3"), value: data.classes["3xx"], tone: "info" },
                { label: tr("ov.s4"), value: data.classes["4xx"], tone: "warn" },
                { label: tr("ov.s5"), value: data.classes["5xx"], tone: "bad" },
              ]}
            />
          )}
        </div>
        <div className="card">
          <h2>{tr("ov.latency")}</h2>
          {data && (
            <Histogram
              bins={data.latency_histogram.map((b) => ({ label: b.le === null ? `>${(2.5).toLocaleString(loc())}s` : b.le >= 1000 ? `${(b.le / 1000).toLocaleString(loc())}s` : `${b.le}`, count: b.count }))}
            />
          )}
          <p className="muted small-text mt">{tr("ov.latNote")}</p>
        </div>
        <div className="card">
          <h2>{tr("ov.diskCache")}</h2>
          <Meter value={cache.bytes} max={cache.max_bytes}>
            <strong>{fmtBytes(cache.bytes)}</strong> <span className="muted">{tr("ov.of", { max: fmtBytes(cache.max_bytes) })}</span>
          </Meter>
          <dl className="kv">
            <dt>{tr("ov.objects")}</dt>
            <dd>{fmtNum(cache.entries)}</dd>
            <dt>{tr("ov.inflight")}</dt>
            <dd>{fmtNum(cache.inflight)}</dd>
          </dl>
        </div>
      </div>

        </>
      )}

      <div className="stats">
        {can("domains:read") && <Kpi label={tr("ov.domVerified")} value={`${verified}/${domains.length}`} />}
        {can("buckets:read") && <Kpi label={tr("nav.buckets")} value={String(buckets.length)} />}
        {can("routes:read") && <Kpi label={tr("ov.activeRoutes")} value={String(state.live_routes.length)} />}
        <Kpi label={tr("ov.config")} value={`#${state.config_version}`} />
      </div>

      {canMetrics && (
        <>
      <div className="card">
        <h2>{tr("ov.perRoute")}</h2>
        <DataTable
          columns={routeColumns()}
          rows={data?.routes ?? []}
          rowKey={(r) => r.id}
          empty={tr("ov.noTraffic")}
          initialSort={{ key: "req", dir: "desc" }}
        />
      </div>

      <div className="card">
        <h2>{tr("ov.topFiles")}</h2>
        <DataTable
          columns={fileColumns()}
          rows={data?.top_files ?? []}
          rowKey={(f) => `${f.route}\0${f.path}`}
          empty={tr("ov.noFiles")}
          initialSort={{ key: "req", dir: "desc" }}
        />
        <p className="muted small-text mt">{tr("ov.topNote")}</p>
      </div>

        </>
      )}

      <p className="muted foot">
        {tr("ov.foot", { v: state.version, c: state.config_version, http: state.http_port, https: state.https_port })}
      </p>
    </Page>
  );
}
