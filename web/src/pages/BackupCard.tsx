import { useRef, useState } from "react";
import { ApiError } from "../api";
import { useAuth, needScope } from "../auth";
import { loc, tr } from "../i18n";
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
      setMsg({ ok: true, text: tr("bk.done") });
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
      setMsg({ ok: true, text: tr("bk.restored") });
      setTimeout(() => window.location.reload(), 6000);
    });

  return (
    <div className="card" id="backup">
      <h2>{tr("bk.title")}</h2>
      <p className="muted">
        {tr("bk.lead")}
      </p>
      {!allowed && <div className="box">{tr("bk.needScope")}</div>}
      <Field label={tr("bk.pass")}>
        <input type="password" autoComplete="new-password" value={pass} onChange={(e) => setPass(e.target.value)} disabled={!allowed} />
      </Field>
      <div className="nav">
        <span />
        <button className="secondary" onClick={download} disabled={busy || !allowed || pass.length < 12} title={allowed ? undefined : needScope("users:manage")}>
          {tr("bk.download")}
        </button>
      </div>
      <Field label={tr("bk.fromFile")}>
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
          {tr("bk.info", { version: info.version, date: new Date(info.created_at * 1000).toLocaleString(loc()), n: info.files.length })}
        </div>
      )}
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
      <div className="nav">
        <span />
        {info ? (
          <button className="primary" onClick={apply} disabled={busy}>
            {tr("bk.restoreNow")}
          </button>
        ) : (
          <button className="secondary" onClick={check} disabled={busy || !allowed || !file || pass.length < 12}>
            {tr("bk.check")}
          </button>
        )}
      </div>
    </div>
  );
}
