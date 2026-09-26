import { useEffect, useRef, useState } from "react";
import { api, type DiagResult, type DiagStep } from "../api";
import { takeDiagnosisTarget } from "../diagnosisTarget";
import { useT, type Key } from "../i18n";
import { Page } from "../ui";

const MARK: Record<DiagStep["status"], { icon: string; cls: string }> = {
  ok: { icon: "✓", cls: "good" },
  warn: { icon: "!", cls: "warn" },
  fail: { icon: "✕", cls: "bad" },
  skip: { icon: "–", cls: "" },
};

export function Diagnosis() {
  const t = useT();
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
      setErr(t("diag.copyFail"));
    }
  };

  const failed = res?.steps.some((s) => s.status === "fail");
  const warned = res?.steps.some((s) => s.status === "warn");

  return (
    <Page
      title={t("diag.title")}
      lead={t("diag.lead")}
    >
      <div className="card">
        <div className="inline tight">
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://cdn.example.com/foto/barca.jpg"
            aria-label={t("diag.aria")}
            spellCheck={false}
            onKeyDown={(e) => e.key === "Enter" && url.trim() && !busy && void run(url.trim())}
          />
          <button className="primary" onClick={() => run(url.trim())} disabled={busy || !url.trim()}>
            {busy ? t("diag.checking") : t("diag.run")}
          </button>
        </div>
        <p className="muted small-text">{t("diag.note")}</p>
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
                  <span className={`diag-mark ${m.cls}`} aria-label={t(`diag.${s.status}` as Key)}>
                    {m.icon}
                  </span>
                  <div>
                    <strong>{s.label}</strong>
                    <div className="muted">{s.detail}</div>
                    {s.fix && (
                      <div className="diag-fix">
                        <strong>{t("diag.todo")}</strong> {s.fix}
                      </div>
                    )}
                  </div>
                </li>
              );
            })}
          </ol>
          <div className="nav">
            <span className="muted small-text">{t("diag.noSecrets")}</span>
            <button className="secondary" onClick={copy}>
              {copied ? t("diag.copied") : t("diag.copy")}
            </button>
          </div>
        </div>
      )}
    </Page>
  );
}
