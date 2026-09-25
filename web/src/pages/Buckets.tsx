import { useState } from "react";
import { api, type BucketInfo, type PanelState, type TestResult } from "../api";
import { useAuth } from "../auth";
import { DataTable, type Column } from "../DataTable";
import { DeleteButton, EmptyState, Field, Modal, Page } from "../ui";

interface Form {
  name: string;
  endpoint: string;
  region: string;
  addressing: "path" | "virtual";
  allow_private_endpoint: boolean;
  access_key: string;
  secret_key: string;
  bucket: string;
  file: string;
}
const EMPTY: Form = {
  name: "",
  endpoint: "",
  region: "us-east-1",
  addressing: "path",
  allow_private_endpoint: false,
  access_key: "",
  secret_key: "",
  bucket: "",
  file: "",
};

const looksPrivate = (endpoint: string) => {
  try {
    const h = new URL(endpoint).hostname;
    return (
      h === "localhost" ||
      h.endsWith(".localhost") ||
      h.endsWith(".local") ||
      /^(127\.|10\.|192\.168\.|169\.254\.|172\.(1[6-9]|2\d|3[01])\.)/.test(h)
    );
  } catch {
    return false;
  }
};

export function Buckets(props: { state: PanelState; refresh: () => Promise<void> }) {
  const { state, refresh } = props;
  const { buckets, rules } = state.panel;
  const { can } = useAuth();
  const canWrite = can("buckets:write");
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const run = async (key: string, fn: () => Promise<unknown>) => {
    setBusy(key);
    setErr(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const columns: Column<BucketInfo>[] = [
    { key: "name", header: "Nome", sort: (x) => x.name, render: (x) => <strong>{x.name}</strong> },
    { key: "bucket", header: "Bucket", sort: (x) => x.bucket, render: (x) => <code>{x.bucket}</code> },
    {
      key: "status",
      header: "Stato",
      sort: (x) => bucketBadge(x).label,
      render: (x) => {
        const s = bucketBadge(x);
        return <span className={s.cls}>{s.label}</span>;
      },
    },
    {
      key: "rules",
      header: "Instradamenti",
      sort: (x) => rules.filter((r) => r.bucket_id === x.id).length,
      align: "right",
      render: (x) => rules.filter((r) => r.bucket_id === x.id).length,
    },
  ];

  return (
    <Page
      title="Bucket"
     
      lead="Censisci gli storage S3 da cui leggere i file. Le chiavi restano su questo nodo e non vengono mai mostrate. Gli instradamenti si creano dopo, scegliendo tra i bucket censiti."
    >
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>Bucket censiti</h2>
          {canWrite && buckets.length > 0 && (
            <button className="primary small" onClick={() => setAdding(true)}>
              Nuovo bucket
            </button>
          )}
        </div>
        <DataTable
          columns={columns}
          rows={buckets}
          rowKey={(x) => x.id}
          empty={
            <EmptyState
              image="bucket"
              title="Nessun bucket collegato"
              text="Collega uno storage S3: le chiavi restano su questo nodo e non vengono mai mostrate."
              action={canWrite ? { label: "Nuovo bucket", onClick: () => setAdding(true) } : undefined}
            />
          }
          searchText={(x) => `${x.name} ${x.bucket} ${x.endpoint}`}
          searchPlaceholder="Cerca bucket…"
          initialSort={{ key: "name", dir: "asc" }}
          defaultExpanded={(x) => !!x.check && x.check.outcome !== "found"}
          actions={!canWrite ? undefined : (x) => (
            <>
              <button className="secondary small" onClick={() => run(`check:${x.id}`, () => api.checkBucket(x.id))} disabled={busy === `check:${x.id}`}>
                {busy === `check:${x.id}` ? "Controllo…" : "Ricontrolla"}
              </button>
              <DeleteButton onConfirm={() => run(`del:${x.id}`, () => api.deleteBucket(x.id))} />
            </>
          )}
          expand={(x) => (
            <div className="details">
              <dl className="kv">
                <dt>Endpoint</dt>
                <dd>
                  <code>{x.endpoint}</code>
                </dd>
                <dt>Regione</dt>
                <dd>{x.region}</dd>
                <dt>Indirizzamento</dt>
                <dd>{x.addressing === "path" ? "Path (endpoint/bucket/file)" : "Virtual host (bucket.endpoint/file)"}</dd>
                <dt>File di prova</dt>
                <dd>
                  <code>{x.test_file || "—"}</code>
                </dd>
              </dl>
              {x.check && <div className={`box ${x.check.outcome === "found" ? "good" : x.check.ok ? "warn" : "bad"}`}>{x.check.message}</div>}
            </div>
          )}
        />
      </div>

      {adding && (
        <NewBucket
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

function bucketBadge(b: BucketInfo) {
  const c = b.check;
  if (!c) return { cls: "badge", label: "Non verificato" };
  if (c.outcome === "found") return { cls: "badge good", label: "Raggiungibile" };
  return c.ok ? { cls: "badge warn", label: "File non trovato" } : { cls: "badge bad", label: "Errore" };
}

export function BucketForm(props: { onCancel?: () => void; cancelLabel?: string; onSaved: () => Promise<void> }) {
  const [f, setF] = useState<Form>(EMPTY);
  const [test, setTest] = useState<{ key: string; result: TestResult } | null>(null);
  const [busy, setBusy] = useState<"test" | "save" | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const set = <K extends keyof Form>(k: K, v: Form[K]) => setF((p) => ({ ...p, [k]: v }));

  const key = JSON.stringify([f.endpoint, f.region, f.addressing, f.allow_private_endpoint, f.access_key, f.secret_key, f.bucket, f.file]);
  const fresh = test && test.key === key ? test.result : null;
  const storage = {
    endpoint: f.endpoint,
    region: f.region,
    addressing: f.addressing,
    allow_private_endpoint: f.allow_private_endpoint,
    access_key: f.access_key,
    secret_key: f.secret_key,
  };
  const canTest = /^https?:\/\/[^/\s]+\/?$/.test(f.endpoint.trim()) && f.access_key && f.secret_key && f.bucket.trim() && f.file.trim();

  const runTest = async () => {
    setBusy("test");
    setErr(null);
    try {
      const result = await api.testBucket({ ...storage, bucket: f.bucket.trim(), prefix: "", file: f.file });
      setTest({ key, result });
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const save = async () => {
    setBusy("save");
    setErr(null);
    try {
      await api.addBucket({ ...storage, name: f.name, bucket: f.bucket.trim(), file: f.file });
      await props.onSaved();
    } catch (e) {
      setErr((e as Error).message);
      setBusy(null);
    }
  };

  return (
    <>
      <Field label="Nome" hint="Per riconoscerlo, ad esempio “Catalogo prodotti”.">
        <input value={f.name} onChange={(e) => set("name", e.target.value)} placeholder="Catalogo prodotti" autoFocus />
      </Field>
      <Field label="Endpoint" hint="Solo indirizzo, senza bucket né percorso.">
        <input
          value={f.endpoint}
          onChange={(e) => {
            set("endpoint", e.target.value);
            if (looksPrivate(e.target.value)) set("allow_private_endpoint", true);
          }}
          placeholder="https://s3.eu-central-1.amazonaws.com"
          spellCheck={false}
          autoComplete="off"
        />
      </Field>
      <div className="row">
        <Field label="Regione">
          <input value={f.region} onChange={(e) => set("region", e.target.value)} spellCheck={false} />
        </Field>
        <Field label="Indirizzamento" hint={f.addressing === "path" ? "endpoint/bucket/file" : "bucket.endpoint/file"}>
          <select value={f.addressing} onChange={(e) => set("addressing", e.target.value as Form["addressing"])}>
            <option value="path">Path (MinIO, Garage…)</option>
            <option value="virtual">Virtual host (AWS, Wasabi…)</option>
          </select>
        </Field>
      </div>
      <div className="row">
        <Field label="Access key">
          <input value={f.access_key} onChange={(e) => set("access_key", e.target.value)} autoComplete="off" spellCheck={false} />
        </Field>
        <Field label="Secret key">
          <input type="password" value={f.secret_key} onChange={(e) => set("secret_key", e.target.value)} autoComplete="new-password" />
        </Field>
      </div>
      {looksPrivate(f.endpoint) && (
        <label className="check">
          <input type="checkbox" checked={f.allow_private_endpoint} onChange={(e) => set("allow_private_endpoint", e.target.checked)} />
          <span>Lo storage è in rete locale o su questa macchina. Consenti l’accesso a un indirizzo interno.</span>
        </label>
      )}
      <div className="row">
        <Field label="Bucket">
          <input value={f.bucket} onChange={(e) => set("bucket", e.target.value)} placeholder="catalogo" spellCheck={false} />
        </Field>
        <Field label="File di prova" hint="Percorso di un file esistente nel bucket: serve a verificare la lettura, ora e nei controlli futuri.">
          <input value={f.file} onChange={(e) => set("file", e.target.value)} placeholder="foto/barca.jpg" spellCheck={false} />
        </Field>
      </div>

      <button className="secondary" onClick={runTest} disabled={!canTest || busy !== null}>
        {busy === "test" ? "Verifico…" : "Verifica connessione"}
      </button>
      {fresh && (
        <div className={`box ${fresh.outcome === "found" ? "good" : fresh.ok ? "warn" : "bad"}`} role="status">
          {fresh.message}
          {fresh.outcome === "found" && (
            <div className="muted">
              {fresh.content_type || "tipo sconosciuto"}
              {fresh.size ? ` · ${fresh.size} byte` : ""}
            </div>
          )}
        </div>
      )}
      {err && <div className="box bad">{err}</div>}
      <div className="nav">
        {props.onCancel ? (
          <button className="ghost" onClick={props.onCancel}>
            {props.cancelLabel ?? "Annulla"}
          </button>
        ) : (
          <span />
        )}
        <button className="primary" onClick={save} disabled={!fresh || !fresh.ok || !f.name.trim() || busy !== null}>
          {busy === "save" ? "Salvo…" : fresh?.outcome === "not_found" ? "Censisci comunque" : "Censisci bucket"}
        </button>
      </div>
    </>
  );
}

function NewBucket(props: { onClose: () => void; onSaved: () => Promise<void> }) {
  return (
    <Modal title="Nuovo bucket" onClose={props.onClose}>
      <BucketForm onCancel={props.onClose} onSaved={props.onSaved} />
    </Modal>
  );
}
