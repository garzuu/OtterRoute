import { useState } from "react";
import { api, domainStatus, type PanelState, type ProbeResult, type RuleInfo, type WarmResult } from "../api";
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
      header: "Indirizzo",
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
      header: "Origine",
      sort: (x) => `${bucketOf(x)?.bucket ?? ""}/${x.folder}`,
      render: (x) => (
        <code>
          {bucketOf(x)?.bucket ?? "?"}/{x.folder}
        </code>
      ),
    },
    {
      key: "storage",
      header: "Bucket censito",
      sort: (x) => bucketOf(x)?.name ?? "",
      render: (x) => bucketOf(x)?.name ?? <span className="muted">—</span>,
    },
    {
      key: "domain",
      header: "Dominio",
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
    <Page title="Instradamenti" lead="Collega un dominio verificato (e un prefisso) a un bucket censito e a una cartella.">
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>Instradamenti attivi</h2>
          {canWrite && rules.length > 0 && (
            <button className="primary small" onClick={() => setAdding(true)}>
              Nuovo instradamento
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
              title="Ancora nessun instradamento"
              text="Collega un dominio verificato a un bucket: da quel momento i file vengono serviti con la cache."
              action={canWrite ? { label: "Nuovo instradamento", onClick: () => setAdding(true) } : undefined}
            />
          }
          searchText={(x) => `${x.domain}${x.path_prefix} ${bucketOf(x)?.name ?? ""} ${bucketOf(x)?.bucket ?? ""} ${x.folder}`}
          searchPlaceholder="Cerca instradamento…"
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
              Serve almeno un <strong>dominio verificato</strong>
              {pending > 0 ? ` (${pending} in attesa)` : ""}.{" "}
              <button className="link" onClick={() => go("domains")}>
                Vai ai domini
              </button>
            </div>
          )}
          {buckets.length === 0 && (
            <div>
              Serve almeno un <strong>bucket</strong>.{" "}
              <button className="link" onClick={() => go("buckets")}>
                Vai ai bucket
              </button>
            </div>
          )}
        </div>
      ) : (
        <>
          <div className="row">
            <Field label="Dominio" hint={pending > 0 ? `${pending} dominio/i in attesa di verifica non compaiono.` : undefined}>
              <select value={dom} onChange={(e) => setDomain(e.target.value)}>
                {verified.map((d) => (
                  <option key={d.host} value={d.host}>
                    {d.host}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Prefisso del percorso" hint="/ = tutto il dominio.">
              <input value={prefix} onChange={(e) => setPrefix(e.target.value)} placeholder="/" spellCheck={false} />
            </Field>
          </div>
          <div className="row">
            <Field label="Bucket">
              <select value={bkt} onChange={(e) => setBucketId(e.target.value)}>
                {buckets.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.name} ({b.bucket})
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Cartella (opzionale)" hint="Tutto ciò che sta fuori resta privato.">
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
            {props.cancelLabel ?? "Annulla"}
          </button>
        ) : (
          <span />
        )}
        <button className="primary" onClick={create} disabled={blocked || busy || !dom || !bkt}>
          {busy ? "Creo…" : "Crea instradamento"}
        </button>
      </div>
    </>
  );
}

function NewRule(props: { state: PanelState; go: (p: PageId) => void; onClose: () => void; onSaved: () => Promise<void> }) {
  return (
    <Modal title="Nuovo instradamento" onClose={props.onClose}>
      <RuleForm state={props.state} go={props.go} onCancel={props.onClose} onSaved={props.onSaved} />
    </Modal>
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
      <strong>Cache</strong>
      <div className="inline tight">
        <input value={file} onChange={(e) => setFile(e.target.value)} placeholder="file da svuotare, es. barca.jpg" aria-label="File da svuotare" spellCheck={false} />
        <button
          className="secondary small"
          disabled={busy !== null || !file.trim()}
          onClick={() =>
            run("file", async () => {
              const res = await api.purgeCache(r.id, full(file));
              setMsg({ ok: true, text: res.removed ? "Copia rimossa: la prossima richiesta la rilegge dallo storage." : "Quel file non era in cache." });
            })
          }
        >
          Svuota file
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
                  setMsg({ ok: true, text: "Cache dell’instradamento svuotata." });
                })
              }
            >
              Conferma
            </button>
            <button className="ghost small" onClick={() => setArmed(false)}>
              Annulla
            </button>
          </>
        ) : (
          <button className="secondary small" onClick={() => setArmed(true)} disabled={busy !== null}>
            Svuota tutto
          </button>
        )}
      </div>
      <textarea rows={3} value={list} onChange={(e) => setList(e.target.value)} placeholder={"file da precaricare, uno per riga (max 200)\nbarca.jpg\nfoto/mare.jpg"} spellCheck={false} aria-label="File da precaricare" />
      <div className="inline tight">
        <button
          className="secondary small"
          disabled={busy !== null || files.length === 0 || files.length > 200}
          onClick={() =>
            run("warm", async () => {
              const res = await api.warmCache(r.id, files.map(full));
              setWarm(res.results);
              const bad = res.results.filter((x) => x.error || (x.status ?? 500) >= 400).length;
              setMsg({ ok: bad === 0, text: bad === 0 ? `${res.results.length} file precaricati.` : `${bad} su ${res.results.length} con problemi.` });
            })
          }
        >
          {busy === "warm" ? "Precarico…" : "Precarica"}
        </button>
        <span className="muted small-text">Il nodo richiede i file uno alla volta: la prima visita sarà già dalla cache.</span>
      </div>
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
      {warm.length > 0 && (
        <table className="mini">
          <thead>
            <tr>
              <th>File</th>
              <th>Stato</th>
              <th>Cache</th>
              <th>Byte</th>
              <th>Tempo</th>
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
          placeholder="file da provare, es. barca.jpg"
          aria-label="File da provare"
          spellCheck={false}
          onKeyDown={(e) => e.key === "Enter" && file.trim() && void probe()}
        />
        <button className="secondary small" onClick={probe} disabled={busy || !file.trim()}>
          {busy ? "…" : "Prova"}
        </button>
        {file.trim() && (
          <button className="ghost small" onClick={() => props.onDiagnose(url)} title="Segue il percorso della richiesta e mostra dove si ferma">
            Diagnosi
          </button>
        )}
        {file.trim() && (
          <a className="ghost small btn" href={url} target="_blank" rel="noreferrer">
            Apri
          </a>
        )}
      </div>
      {err && <div className="box bad">{err}</div>}
      {runs.length > 0 && (
        <table className="mini">
          <thead>
            <tr>
              <th>#</th>
              <th>Stato</th>
              <th>Cache</th>
              <th>Tipo</th>
              <th>Tempo</th>
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
