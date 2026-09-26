import { useState } from "react";
import { api, domainStatus, type PanelState, type ProbeResult, type RuleInfo, type WarmResult } from "../api";
import { loc, rich, tr, type Key } from "../i18n";
import { useAuth } from "../auth";
import { DataTable, type Column } from "../DataTable";
import type { PageId } from "../Dashboard";
import { DeleteButton, EmptyState, Field, Modal, Page } from "../ui";
import { DOMAIN_BADGE } from "./Domains";
import { setDiagnosisTarget } from "../diagnosisTarget";

const cleanFolder = (p: string) => p.split("/").filter(Boolean).join("/");
const routePrefix = (p: string) => {
  const s = p.split("/").filter(Boolean).join("/");
  return s ? `/${s}/` : "/";
};

export function Routes(props: { state: PanelState; refresh: () => Promise<void>; go: (p: PageId) => void }) {
  const { state, refresh, go } = props;
  const { domains, buckets, rules } = state.panel;
  const { can } = useAuth();
  const canWrite = can("routes:write");
  const [adding, setAdding] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const bucketOf = (x: RuleInfo) => buckets.find((b) => b.id === x.bucket_id);
  const domainOf = (x: RuleInfo) => domains.find((d) => d.host === x.domain);
  const columns: Column<RuleInfo>[] = [
    {
      key: "address",
      header: tr("rt.colAddress"),
      sort: (x) => `${x.domain}${x.path_prefix}`,
      render: (x) => (
        <code className="big">
          {x.domain}
          {x.path_prefix}
        </code>
      ),
    },
    {
      key: "origin",
      header: tr("rt.colOrigin"),
      sort: (x) => `${bucketOf(x)?.bucket ?? ""}/${x.folder}`,
      render: (x) => (
        <code>
          {bucketOf(x)?.bucket ?? "?"}/{x.folder}
        </code>
      ),
    },
    {
      key: "storage",
      header: tr("rt.colBucket"),
      sort: (x) => bucketOf(x)?.name ?? "",
      render: (x) => bucketOf(x)?.name ?? <span className="muted">—</span>,
    },
    {
      key: "access",
      header: tr("rt.colAccess"),
      sort: (x) => (x.signed ? "1" : "0"),
      render: (x) => (
        <>
          {x.signed ? <span className="badge warn">{tr("rt.signed")}</span> : <span className="badge">{tr("rt.public")}</span>}{" "}
          {x.images && <span className="badge good">{tr("rt.images")}</span>}
        </>
      ),
    },
    {
      key: "domain",
      header: tr("dm.colDomain"),
      sort: (x) => {
        const d = domainOf(x);
        return d ? DOMAIN_BADGE[domainStatus(d)].label : "";
      },
      render: (x) => {
        const d = domainOf(x);
        if (!d) return <span className="muted">—</span>;
        const b = DOMAIN_BADGE[domainStatus(d)];
        return <span className={b.cls}>{b.label}</span>;
      },
    },
  ];

  return (
    <Page title={tr("nav.routes")} lead={tr("rt.lead")}>
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>{tr("rt.active")}</h2>
          {canWrite && rules.length > 0 && (
            <button className="primary small" onClick={() => setAdding(true)}>
              {tr("rt.new")}
            </button>
          )}
        </div>
        <DataTable
          columns={columns}
          rows={rules}
          rowKey={(x) => x.id}
          empty={
            <EmptyState
              image="laptop"
              title={tr("rt.none")}
              text={tr("rt.noneText")}
              action={canWrite ? { label: tr("rt.new"), onClick: () => setAdding(true) } : undefined}
            />
          }
          searchText={(x) => `${x.domain}${x.path_prefix} ${bucketOf(x)?.name ?? ""} ${bucketOf(x)?.bucket ?? ""} ${x.folder}`}
          searchPlaceholder={tr("rt.search")}
          initialSort={{ key: "address", dir: "asc" }}
          actions={!canWrite ? undefined : (x) => (
            <DeleteButton
              onConfirm={async () => {
                try {
                  await api.deleteRule(x.id);
                  setErr(null);
                  await refresh();
                } catch (e) {
                  setErr((e as Error).message);
                }
              }}
            />
          )}
          expand={(x) => (
            <>
              <ProbePanel r={x} port={state.http_port} onDiagnose={(u) => { setDiagnosisTarget(u); go("diagnosis"); }} />
              {canWrite && <LinksPanel r={x} refresh={refresh} />}
              {canWrite && <CachePanel r={x} />}
            </>
          )}
        />
      </div>

      {adding && (
        <NewRule
          state={state}
          go={go}
          onClose={() => setAdding(false)}
          onSaved={async () => {
            setAdding(false);
            await refresh();
          }}
        />
      )}
    </Page>
  );
}

export function RuleForm(props: { state: PanelState; go: (p: PageId) => void; onCancel?: () => void; cancelLabel?: string; onSaved: () => Promise<void> }) {
  const { state, go } = props;
  const { domains, buckets } = state.panel;
  const verified = domains.filter((d) => d.verified);
  const pending = domains.length - verified.length;

  const [domain, setDomain] = useState("");
  const [prefix, setPrefix] = useState("/");
  const [bucketId, setBucketId] = useState("");
  const [folder, setFolder] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const dom = domain || verified[0]?.host || "";
  const bkt = bucketId || buckets[0]?.id || "";
  const bucket = buckets.find((b) => b.id === bkt);
  const blocked = verified.length === 0 || buckets.length === 0;

  const create = async () => {
    setBusy(true);
    setErr(null);
    try {
      await api.addRule({ domain: dom, path_prefix: prefix, bucket_id: bkt, folder });
      await props.onSaved();
    } catch (e) {
      setErr((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <>
      {blocked ? (
        <div className="box warn">
          {verified.length === 0 && (
            <div>
              {rich(tr("rt.needDomain", { pending: pending > 0 ? tr("rt.pendingN", { n: pending }) : "" }))}
              <button className="link" onClick={() => go("domains")}>
                {tr("rt.goDomains")}
              </button>
            </div>
          )}
          {buckets.length === 0 && (
            <div>
              {rich(tr("rt.needBucket"))}
              <button className="link" onClick={() => go("buckets")}>
                {tr("rt.goBuckets")}
              </button>
            </div>
          )}
        </div>
      ) : (
        <>
          <div className="row">
            <Field label={tr("dm.colDomain")} hint={pending > 0 ? tr("rt.pendingHint", { n: pending }) : undefined}>
              <select value={dom} onChange={(e) => setDomain(e.target.value)}>
                {verified.map((d) => (
                  <option key={d.host} value={d.host}>
                    {d.host}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={tr("rt.prefix")} hint={tr("rt.prefixHint")}>
              <input value={prefix} onChange={(e) => setPrefix(e.target.value)} placeholder="/" spellCheck={false} />
            </Field>
          </div>
          <div className="row">
            <Field label={tr("nav.buckets")}>
              <select value={bkt} onChange={(e) => setBucketId(e.target.value)}>
                {buckets.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.name} ({b.bucket})
                  </option>
                ))}
              </select>
            </Field>
            <Field label={tr("rt.folder")} hint={tr("rt.folderHint")}>
              <input value={folder} onChange={(e) => setFolder(e.target.value)} placeholder="foto/" spellCheck={false} />
            </Field>
          </div>
          <div className="map">
            <code>
              {dom}
              {routePrefix(prefix || "/")}
              <b>file.jpg</b>
            </code>
            <span aria-hidden>→</span>
            <code>
              {bucket?.bucket}/{cleanFolder(folder) ? `${cleanFolder(folder)}/` : ""}
              <b>file.jpg</b>
            </code>
          </div>
        </>
      )}
      {err && (
        <div className="box bad">
          <pre>{err}</pre>
        </div>
      )}
      <div className="nav">
        {props.onCancel ? (
          <button className="ghost" onClick={props.onCancel}>
            {props.cancelLabel ?? tr("common.cancel")}
          </button>
        ) : (
          <span />
        )}
        <button className="primary" onClick={create} disabled={blocked || busy || !dom || !bkt}>
          {busy ? tr("rt.creating") : tr("rt.create")}
        </button>
      </div>
    </>
  );
}

function NewRule(props: { state: PanelState; go: (p: PageId) => void; onClose: () => void; onSaved: () => Promise<void> }) {
  return (
    <Modal title={tr("rt.new")} onClose={props.onClose}>
      <RuleForm state={props.state} go={props.go} onCancel={props.onClose} onSaved={props.onSaved} />
    </Modal>
  );
}

const TTLS: [Key, number][] = [
  ["rt.ttl300", 300],
  ["rt.ttl3600", 3600],
  ["rt.ttl86400", 86400],
  ["rt.ttl604800", 604800],
  ["rt.ttl2592000", 2592000],
];

/** Link firmati con scadenza: attivazione per instradamento e creazione dei link. */
function LinksPanel(props: { r: RuleInfo; refresh: () => Promise<void> }) {
  const { r } = props;
  const { can } = useAuth();
  const [file, setFile] = useState("");
  const [ttl, setTtl] = useState(3600);
  const [https, setHttps] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [link, setLink] = useState<{ url: string; expires_at: number } | null>(null);
  const [copied, setCopied] = useState(false);
  const [armed, setArmed] = useState(false);
  const full = `${r.path_prefix}${file.trim().replace(/^\//, "")}`;

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setMsg(null);
    try {
      await fn();
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="details">
      <strong>{tr("rt.accessImages")}</strong>
      <label className="check">
        <input
          type="checkbox"
          checked={r.signed}
          disabled={busy}
          onChange={(e) =>
            run(async () => {
              await api.setRuleOptions(r.id, { signed: e.target.checked });
              setLink(null);
              await props.refresh();
            })
          }
        />
        <span>
          {rich(tr("rt.signedText"))}
        </span>
      </label>
      <label className="check">
        <input
          type="checkbox"
          checked={r.images}
          disabled={busy}
          onChange={(e) =>
            run(async () => {
              await api.setRuleOptions(r.id, { images: e.target.checked });
              await props.refresh();
            })
          }
        />
        <span>
          {rich(tr("rt.imagesText"))}
        </span>
      </label>
      {r.signed && (
        <>
          <div className="inline tight">
            <input value={file} onChange={(e) => setFile(e.target.value)} placeholder={tr("rt.filePh")} aria-label={tr("rt.linkFileAria")} spellCheck={false} />
            <select value={ttl} onChange={(e) => setTtl(Number(e.target.value))} aria-label={tr("rt.validityAria")} style={{ width: "auto" }}>
              {TTLS.map(([l, s]) => (
                <option key={s} value={s}>
                  {tr("rt.valid", { l: tr(l) })}
                </option>
              ))}
            </select>
            <button
              className="secondary small"
              disabled={busy || !file.trim()}
              onClick={() =>
                run(async () => {
                  setLink(await api.createLink({ rule: r.id, path: full, ttl_secs: ttl, https }));
                  setCopied(false);
                })
              }
            >
              {tr("rt.makeLink")}
            </button>
          </div>
          <label className="check">
            <input type="checkbox" checked={https} onChange={(e) => setHttps(e.target.checked)} />
            <span>{tr("rt.https")}</span>
          </label>
          {link && (
            <div className="box good">
              <div>
                {rich(tr("rt.expires", { date: new Date(link.expires_at * 1000).toLocaleString(loc()) }))}
              </div>
              <div className="inline tight">
                <input readOnly value={link.url} aria-label={tr("rt.linkAria")} onFocus={(e) => e.currentTarget.select()} />
                <button
                  className="secondary small"
                  onClick={async () => {
                    try {
                      await navigator.clipboard.writeText(link.url);
                      setCopied(true);
                    } catch {
                      setMsg({ ok: false, text: tr("rt.copyFail") });
                    }
                  }}
                >
                  {copied ? tr("rt.copied") : tr("common.copy")}
                </button>
              </div>
            </div>
          )}
          {can("settings:write") && (
            <div className="inline tight">
              {armed ? (
                <>
                  <button
                    className="danger small"
                    disabled={busy}
                    onClick={() =>
                      run(async () => {
                        await api.rotateLinks();
                        setArmed(false);
                        setLink(null);
                        setMsg({ ok: true, text: tr("rt.rotated") });
                      })
                    }
                  >
                    {tr("rt.rotateConfirm")}
                  </button>
                  <button className="ghost small" onClick={() => setArmed(false)}>
                    {tr("common.cancel")}
                  </button>
                </>
              ) : (
                <button className="ghost small" onClick={() => setArmed(true)} disabled={busy}>
                  {tr("rt.rotate")}
                </button>
              )}
              <span className="muted small-text">{tr("rt.rotateNote")}</span>
            </div>
          )}
        </>
      )}
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
    </div>
  );
}

/** Svuotamento e precaricamento della cache di un instradamento. */
function CachePanel(props: { r: RuleInfo }) {
  const { r } = props;
  const [file, setFile] = useState("");
  const [list, setList] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [warm, setWarm] = useState<WarmResult[]>([]);
  const [armed, setArmed] = useState(false);
  const full = (f: string) => `${r.path_prefix}${f.trim().replace(/^\//, "")}`;

  const run = async (what: string, fn: () => Promise<void>) => {
    setBusy(what);
    setMsg(null);
    try {
      await fn();
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(null);
    }
  };

  const files = list
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

  return (
    <div className="details">
      <strong>{tr("rt.cache")}</strong>
      <div className="inline tight">
        <input value={file} onChange={(e) => setFile(e.target.value)} placeholder={tr("rt.purgePh")} aria-label={tr("rt.purgeAria")} spellCheck={false} />
        <button
          className="secondary small"
          disabled={busy !== null || !file.trim()}
          onClick={() =>
            run("file", async () => {
              const res = await api.purgeCache(r.id, full(file));
              setMsg({ ok: true, text: res.removed ? tr("rt.removed") : tr("rt.notCached") });
            })
          }
        >
          {tr("rt.purgeFile")}
        </button>
        {armed ? (
          <>
            <button
              className="danger small"
              disabled={busy !== null}
              onClick={() =>
                run("all", async () => {
                  await api.purgeCache(r.id);
                  setArmed(false);
                  setMsg({ ok: true, text: tr("rt.purgedAll") });
                })
              }
            >
              {tr("common.confirm")}
            </button>
            <button className="ghost small" onClick={() => setArmed(false)}>
              {tr("common.cancel")}
            </button>
          </>
        ) : (
          <button className="secondary small" onClick={() => setArmed(true)} disabled={busy !== null}>
            {tr("rt.purgeAll")}
          </button>
        )}
      </div>
      <textarea rows={3} value={list} onChange={(e) => setList(e.target.value)} placeholder={tr("rt.warmPh")} spellCheck={false} aria-label={tr("rt.warmAria")} />
      <div className="inline tight">
        <button
          className="secondary small"
          disabled={busy !== null || files.length === 0 || files.length > 200}
          onClick={() =>
            run("warm", async () => {
              const res = await api.warmCache(r.id, files.map(full));
              setWarm(res.results);
              const bad = res.results.filter((x) => x.error || (x.status ?? 500) >= 400).length;
              setMsg({ ok: bad === 0, text: bad === 0 ? tr("rt.warmed", { n: res.results.length }) : tr("rt.warmBad", { bad, n: res.results.length }) });
            })
          }
        >
          {busy === "warm" ? tr("rt.warming") : tr("rt.warm")}
        </button>
        <span className="muted small-text">{tr("rt.warmNote")}</span>
      </div>
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
      {warm.length > 0 && (
        <table className="mini">
          <thead>
            <tr>
              <th>{tr("rt.thFile")}</th>
              <th>{tr("rt.thStatus")}</th>
              <th>{tr("rt.thCache")}</th>
              <th>{tr("rt.thBytes")}</th>
              <th>{tr("rt.thTime")}</th>
            </tr>
          </thead>
          <tbody>
            {warm.map((x) => (
              <tr key={x.path}>
                <td>{x.path}</td>
                <td className={x.status && x.status < 300 ? "ok" : "ko"}>{x.status ?? x.error}</td>
                <td>
                  <span className={`tag ${x.x_cache ?? ""}`}>{x.x_cache ?? "—"}</span>
                </td>
                <td>{x.bytes ?? "—"}</td>
                <td>{x.elapsed_ms != null ? `${x.elapsed_ms} ms` : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function ProbePanel(props: { r: RuleInfo; port: number; onDiagnose: (url: string) => void }) {
  const { r } = props;
  const [file, setFile] = useState("");
  const [runs, setRuns] = useState<ProbeResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const path = `${r.path_prefix}${file.replace(/^\//, "")}`;
  const url = `http://${r.domain}${props.port === 80 ? "" : `:${props.port}`}${path}`;

  const probe = async () => {
    setBusy(true);
    setErr(null);
    try {
      const res = await api.probe(r.domain, path);
      setRuns((p) => [...p.slice(-4), res]);
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="details">
      <div className="inline tight">
        <input
          value={file}
          onChange={(e) => setFile(e.target.value)}
          placeholder={tr("rt.probePh")}
          aria-label={tr("rt.probeAria")}
          spellCheck={false}
          onKeyDown={(e) => e.key === "Enter" && file.trim() && void probe()}
        />
        <button className="secondary small" onClick={probe} disabled={busy || !file.trim()}>
          {busy ? "…" : tr("rt.probe")}
        </button>
        {file.trim() && (
          <button className="ghost small" onClick={() => props.onDiagnose(url)} title={tr("rt.diagnoseTip")}>
            {tr("rt.diagnose")}
          </button>
        )}
        {file.trim() && (
          <a className="ghost small btn" href={url} target="_blank" rel="noreferrer">
            {tr("rt.open")}
          </a>
        )}
      </div>
      {err && <div className="box bad">{err}</div>}
      {runs.length > 0 && (
        <table className="mini">
          <thead>
            <tr>
              <th>#</th>
              <th>{tr("rt.thStatus")}</th>
              <th>{tr("rt.thCache")}</th>
              <th>{tr("rt.thType")}</th>
              <th>{tr("rt.thTime")}</th>
            </tr>
          </thead>
          <tbody>
            {runs.map((x, i) => (
              <tr key={i}>
                <td>{i + 1}</td>
                <td className={x.status < 300 ? "ok" : "ko"}>{x.status}</td>
                <td>
                  <span className={`tag ${x.x_cache ?? ""}`}>{x.x_cache ?? "—"}</span>
                </td>
                <td>{x.content_type ?? "—"}</td>
                <td>{x.elapsed_ms} ms</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
