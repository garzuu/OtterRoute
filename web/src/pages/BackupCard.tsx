import { useRef, useState } from "react";
import { ApiError } from "../api";
import { useAuth, needScope } from "../auth";
import { Field } from "../ui";

interface Manifest {
  version: string;
  created_at: number;
  node_id: string | null;
  files: string[];
}

const HDR = { "X-OtterRoute-Restore": "1", "Content-Type": "application/octet-stream" };

async function fail(res: Response): Promise<never> {
  const d = await res.json().catch(() => ({}));
  throw new ApiError(d.error ?? `Errore ${res.status}`, res.status);
}

export function BackupCard() {
  const { can } = useAuth();
  const allowed = can("users:manage");
  const [pass, setPass] = useState("");
  const [file, setFile] = useState<File | null>(null);
  const [info, setInfo] = useState<Manifest | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const input = useRef<HTMLInputElement>(null);

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

  const download = () =>
    run(async () => {
      const res = await fetch("/api/backup/export", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ passphrase: pass }),
      });
      if (!res.ok) return fail(res);
      const url = URL.createObjectURL(await res.blob());
      const a = document.createElement("a");
      a.href = url;
      a.download = `otterroute-backup-${new Date().toISOString().slice(0, 10)}.otrbak`;
      a.click();
      URL.revokeObjectURL(url);
      setMsg({ ok: true, text: "Backup scaricato. Conserva il file e la frase segreta in luoghi separati: senza la frase non si apre." });
    });

  const body = async () => {
    const p = new TextEncoder().encode(pass);
    const head = new Uint8Array(4);
    new DataView(head.buffer).setUint32(0, p.length);
    return new Blob([head, p, await file!.arrayBuffer()]);
  };

  const check = () =>
    run(async () => {
      setInfo(null);
      const res = await fetch("/api/backup/restore?dry=1", { method: "POST", headers: HDR, body: await body() });
      if (!res.ok) return fail(res);
      setInfo((await res.json()).manifest);
    });

  const apply = () =>
    run(async () => {
      const res = await fetch("/api/backup/restore", { method: "POST", headers: HDR, body: await body() });
      if (!res.ok) return fail(res);
      setInfo(null);
      setMsg({ ok: true, text: "Ripristinato. Il nodo si sta riavviando: tra qualche secondo dovrai accedere di nuovo." });
      setTimeout(() => window.location.reload(), 6000);
    });

  return (
    <div className="card" id="backup">
      <h2>Backup e ripristino</h2>
      <p className="muted">
        Un unico file cifrato con utenti, chiavi, certificati, domini, bucket e notifiche (non la cache). Contiene segreti: la frase serve per aprirlo e non si può recuperare.
      </p>
      {!allowed && <div className="box">Serve lo scope <code>users:manage</code>.</div>}
      <Field label="Frase segreta (almeno 12 caratteri)">
        <input type="password" autoComplete="new-password" value={pass} onChange={(e) => setPass(e.target.value)} disabled={!allowed} />
      </Field>
      <div className="nav">
        <span />
        <button className="secondary" onClick={download} disabled={busy || !allowed || pass.length < 12} title={allowed ? undefined : needScope("users:manage")}>
          Scarica backup
        </button>
      </div>
      <Field label="Ripristina da un file">
        <input
          ref={input}
          type="file"
          accept=".otrbak"
          disabled={!allowed}
          onChange={(e) => {
            setFile(e.target.files?.[0] ?? null);
            setInfo(null);
          }}
        />
      </Field>
      {info && (
        <div className="box warn">
          Backup della versione <strong>{info.version}</strong> del {new Date(info.created_at * 1000).toLocaleString()} ({info.files.length} file). Ripristinando, <strong>lo stato attuale viene sostituito</strong> (una copia resta in <code>backups/pre-restore</code>), il nodo si riavvia e le sessioni si perdono.
        </div>
      )}
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
      <div className="nav">
        <span />
        {info ? (
          <button className="primary" onClick={apply} disabled={busy}>
            Ripristina ora
          </button>
        ) : (
          <button className="secondary" onClick={check} disabled={busy || !allowed || !file || pass.length < 12}>
            Controlla il file
          </button>
        )}
      </div>
    </div>
  );
}
