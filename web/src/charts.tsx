import { useState, type ReactNode } from "react";
import type { MetricsRange, MetricsResponse } from "./api";
import { loc, tr } from "./i18n";
import { fmtBytes } from "./ui";

export const fmtNum = (n: number) => new Intl.NumberFormat(loc()).format(n);
export const fmtPct = (v: number | null) => (v === null ? "—" : `${(v * 100).toLocaleString(loc(), { maximumFractionDigits: 1 })}%`);
/** Le latenze arrivano come limite superiore della fascia; oltre l'ultima fascia il valore è "enorme". */
export const fmtMs = (v: number | null) =>
  v === null ? "—" : v > 1e9 ? "> 2,5 s" : v >= 1000 ? `${(v / 1000).toLocaleString(loc())} s` : `≤ ${v} ms`;

/** Estremo "tondo" per l'asse verticale. */
function niceMax(v: number): number {
  if (v <= 4) return 4;
  const p = 10 ** Math.floor(Math.log10(v));
  const n = v / p;
  return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * p;
}

function timeLabel(t: number, range: MetricsRange): string {
  const d = new Date(t * 1000);
  if (range === "1h") return d.toLocaleTimeString(loc(), { hour: "2-digit", minute: "2-digit" });
  if (range === "24h") return d.toLocaleTimeString(loc(), { hour: "2-digit", minute: "2-digit" });
  return d.toLocaleDateString(loc(), { day: "2-digit", month: "2-digit" });
}

function tipLabel(t: number, range: MetricsRange): string {
  const d = new Date(t * 1000);
  return range === "1h"
    ? d.toLocaleTimeString(loc(), { hour: "2-digit", minute: "2-digit" })
    : d.toLocaleString(loc(), { day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" });
}

/** Richieste nel tempo, a barre impilate: HIT / MISS / altro. */
export function RequestsChart(props: { series: MetricsResponse["series"]; range: MetricsRange }) {
  const { series, range } = props;
  const [hover, setHover] = useState<number | null>(null);
  const W = 800;
  const H = 200;
  const pad = { l: 44, r: 8, t: 8, b: 24 };
  const iw = W - pad.l - pad.r;
  const ih = H - pad.t - pad.b;
  const max = niceMax(Math.max(1, ...series.map((p) => p.req)));
  const bw = iw / series.length;
  const y = (v: number) => pad.t + ih - (v / max) * ih;
  // etichette dell'asse orizzontale: circa sei
  const every = Math.max(1, Math.round(series.length / 6));
  const p = hover === null ? null : series[hover];

  return (
    <div className="chart">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={tr("chart.aria")} onMouseLeave={() => setHover(null)}>
        {[0, 0.25, 0.5, 0.75, 1].map((f) => (
          <g key={f}>
            <line x1={pad.l} x2={W - pad.r} y1={y(max * f)} y2={y(max * f)} className="grid" />
            <text x={pad.l - 8} y={y(max * f) + 4} textAnchor="end" className="axis">
              {fmtNum(Math.round(max * f))}
            </text>
          </g>
        ))}
        {series.map((pt, i) => {
          const x = pad.l + i * bw;
          const w = Math.max(1, bw - Math.min(3, bw * 0.25));
          const hitH = (pt.hit / max) * ih;
          const missH = (pt.miss / max) * ih;
          const otherH = (pt.other / max) * ih;
          const base = pad.t + ih;
          return (
            <g key={pt.t} onMouseEnter={() => setHover(i)}>
              <rect x={x} y={pad.t} width={bw} height={ih} className={hover === i ? "hover-col on" : "hover-col"} />
              <rect x={x + (bw - w) / 2} y={base - hitH} width={w} height={hitH} className="bar-hit" />
              <rect x={x + (bw - w) / 2} y={base - hitH - missH} width={w} height={missH} className="bar-miss" />
              <rect x={x + (bw - w) / 2} y={base - hitH - missH - otherH} width={w} height={otherH} className="bar-other" />
              {i % every === 0 && (
                <text x={x + bw / 2} y={H - 6} textAnchor="middle" className="axis">
                  {timeLabel(pt.t, range)}
                </text>
              )}
            </g>
          );
        })}
      </svg>
      {p && hover !== null && (
        <div className="chart-tip" style={{ left: `${((pad.l + (hover + 0.5) * bw) / W) * 100}%` }}>
          <strong>{tipLabel(p.t, range)}</strong>
          <span>
            <i className="sw hit" /> {tr("chart.hit")} <b>{fmtNum(p.hit)}</b>
          </span>
          <span>
            <i className="sw miss" /> {tr("chart.miss")} <b>{fmtNum(p.miss)}</b>
          </span>
          <span>
            <i className="sw other" /> {tr("chart.other")} <b>{fmtNum(p.other)}</b>
          </span>
          <span className="muted">
            {tr("chart.bandwidth", { bytes: fmtBytes(p.bytes), err: fmtNum(p.err) })}
          </span>
        </div>
      )}
      <div className="legend">
        <span>
          <i className="sw hit" /> {tr("chart.legendHit")}
        </span>
        <span>
          <i className="sw miss" /> {tr("chart.legendMiss")}
        </span>
        <span>
          <i className="sw other" /> {tr("chart.legendOther")}
        </span>
      </div>
    </div>
  );
}

/** Barre orizzontali con valore. */
export function HBars(props: { items: { label: string; value: number; tone: "good" | "info" | "warn" | "bad" }[] }) {
  const total = props.items.reduce((s, i) => s + i.value, 0);
  return (
    <ul className="hbars">
      {props.items.map((i) => (
        <li key={i.label}>
          <span className="hb-label">{i.label}</span>
          <span className="hb-track">
            <span className={`hb-fill ${i.tone}`} style={{ width: total ? `${(i.value / total) * 100}%` : 0 }} />
          </span>
          <span className="hb-val">{fmtNum(i.value)}</span>
        </li>
      ))}
    </ul>
  );
}

/** Istogramma verticale compatto della latenza. */
export function Histogram(props: { bins: { label: string; count: number }[] }) {
  const max = Math.max(1, ...props.bins.map((b) => b.count));
  return (
    <div className="histo" role="img" aria-label="Distribuzione della latenza">
      {props.bins.map((b) => (
        <div key={b.label} className="hi-col" title={`${b.label}: ${fmtNum(b.count)}`}>
          <span className="hi-bar" style={{ height: `${Math.max(b.count ? 4 : 0, (b.count / max) * 100)}%` }} />
          <span className="hi-label">{b.label}</span>
        </div>
      ))}
    </div>
  );
}

/** Barra di avanzamento con etichetta. */
export function Meter(props: { value: number; max: number; children?: ReactNode }) {
  const f = props.max > 0 ? Math.min(1, props.value / props.max) : 0;
  return (
    <div>
      <div className="meter-track">
        <span className={f > 0.9 ? "meter-fill bad" : f > 0.75 ? "meter-fill warn" : "meter-fill"} style={{ width: `${f * 100}%` }} />
      </div>
      {props.children && <div className="meter-note">{props.children}</div>}
    </div>
  );
}
