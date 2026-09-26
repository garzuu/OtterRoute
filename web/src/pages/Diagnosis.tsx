import { useEffect, useRef, useState } from "react";
import { api, type DiagResult, type DiagStep } from "../api";
import { takeDiagnosisTarget } from "../diagnosisTarget";
import { Page } from "../ui";

const MARK: Record<DiagStep["status"], { icon: string; cls: string; text: string }> = {
  ok: { icon: "✓", cls: "good", text: "Ok" },
  warn: { icon: "!", cls: "warn", text: "Attenzione" },
  fail: { icon: "✕", cls: "bad", text: "Problema" },
  skip: { icon: "–", cls: "", text: "Saltato" },
};

export function Diagnosis() {
  const [url, setUrl] = useState("");
  const [res, setRes] = useState<DiagResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const started = useRef(false);

  const run = async (target: string) => {
    setBusy(true);
    setErr(null);
    setCopied(false);
    try {
      setRes(await api.diagnose(target));
    } catch (e) {
      setRes(null);
      setErr((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (started.current) return;
    started.current = true;
    const t = takeDiagnosisTarget();
    if (t) {
      setUrl(t);
      void run(t);
    }
  }, []);

  const copy = async () => {
    if (!res) return;
    try {
      await navigator.clipboard.writeText(res.report);
      setCopied(true);
    } catch {
      setErr("Copia non riuscita: seleziona il testo del report a mano.");
    }
  };

  const failed = res?.steps.some((s) => s.status === "fail");
  const warned = res?.steps.some((s) => s.status === "warn");

  return (
    <Page
      title="Diagnosi"
      lead="Scrivi l’indirizzo di un file che non funziona: il nodo segue il percorso della richiesta (DNS, nodo, instradamento, cache, storage) e ti dice dove si ferma e cosa fare."
    >
      <div className="card">
        <div className="inline tight">
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://cdn.example.com/foto/barca.jpg"
            aria-label="Indirizzo da diagnosticare"
            spellCheck={false}
            onKeyDown={(e) => e.key === "Enter" && url.trim() && !busy && void run(url.trim())}
          />
          <button className="primary" onClick={() => run(url.trim())} disabled={busy || !url.trim()}>
            {busy ? "Controllo…" : "Diagnostica"}
          </button>
        </div>
        <p className="muted small-text">La prova fa una richiesta reale al nodo: se il file esiste finisce anche in cache, come una visita vera.</p>
      </div>

      {err && <div className="box bad">{err}</div>}

      {res && (
        <div className="card">
          <div className={`box ${failed ? "bad" : warned ? "warn" : "good"}`}>{res.summary}</div>
          <ol className="diag">
            {res.steps.map((s) => {
              const m = MARK[s.status];
              return (
                <li key={s.id} className={`diag-step ${m.cls}`}>
                  <span className={`diag-mark ${m.cls}`} aria-label={m.text}>
                    {m.icon}
                  </span>
                  <div>
                    <strong>{s.label}</strong>
                    <div className="muted">{s.detail}</div>
                    {s.fix && (
                      <div className="diag-fix">
                        <strong>Cosa fare:</strong> {s.fix}
                      </div>
                    )}
                  </div>
                </li>
              );
            })}
          </ol>
          <div className="nav">
            <span className="muted small-text">Il report non contiene chiavi né segreti.</span>
            <button className="secondary" onClick={copy}>
              {copied ? "Copiato ✓" : "Copia report"}
            </button>
          </div>
        </div>
      )}
    </Page>
  );
}
