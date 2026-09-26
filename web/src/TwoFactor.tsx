import { useEffect, useState, type FormEvent } from "react";
import { api, type TwoFactorStart } from "./api";
import { IconCheck } from "./icons";
import { tr, useT } from "./i18n";

/** Campo per il codice a 6 cifre dell'app di autenticazione. */
export function CodeField(props: { value: string; onChange: (v: string) => void; autoFocus?: boolean }) {
  return (
    <input
      className="codefield"
      value={props.value}
      onChange={(e) => props.onChange(e.target.value.replace(/[^\d ]/g, "").slice(0, 7))}
      inputMode="numeric"
      autoComplete="one-time-code"
      placeholder="123 456"
      maxLength={7}
      autoFocus={props.autoFocus}
      aria-label={tr("tf.codeAria")}
    />
  );
}

const group = (s: string) => s.replace(/(.{4})/g, "$1 ").trim();

function copy(text: string) {
  try {
    void navigator.clipboard.writeText(text);
  } catch {
    /* senza permessi il testo resta selezionabile */
  }
}

function download(name: string, text: string) {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}

/** Codici di recupero, mostrati una sola volta. */
export function RecoveryCodes(props: { codes: string[]; onDone: () => void; doneLabel?: string }) {
  const t = useT();
  const [saved, setSaved] = useState(false);
  const text = `${t("tf.recoveryHead")}\n\n${props.codes.join("\n")}\n`;
  return (
    <div>
      <div className="box warn">
        <strong>{t("tf.saveNow")}</strong> {t("tf.saveNowText")}
      </div>
      <ul className="rcodes">
        {props.codes.map((c) => (
          <li key={c}>
            <code>{c}</code>
          </li>
        ))}
      </ul>
      <div className="actions">
        <button className="secondary small" onClick={() => copy(props.codes.join("\n"))}>
          {t("common.copy")}
        </button>
        <button className="secondary small" onClick={() => download(t("tf.recoveryFile"), text)}>
          {t("tf.download")}
        </button>
      </div>
      <label className="check mt">
        <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        <span>{t("tf.saved")}</span>
      </label>
      <div className="nav">
        <span />
        <button className="primary" disabled={!saved} onClick={props.onDone}>
          {props.doneLabel ?? t("common.done")}
        </button>
      </div>
    </div>
  );
}

/** Attivazione della 2FA: QR → codice di conferma → codici di recupero. */
export function TwoFactorSetup(props: { onDone: () => void; onCancel?: () => void }) {
  const t = useT();
  const [start, setStart] = useState<TwoFactorStart | null>(null);
  const [codes, setCodes] = useState<string[] | null>(null);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api
      .twoFactorStart()
      .then((s) => live && setStart(s))
      .catch((e: Error) => live && setErr(e.message));
    return () => {
      live = false;
    };
  }, []);

  const confirm = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      setCodes((await api.twoFactorConfirm(code)).recovery_codes);
    } catch (x) {
      setErr((x as Error).message);
    } finally {
      setBusy(false);
    }
  };

  if (codes) return <RecoveryCodes codes={codes} onDone={props.onDone} doneLabel={t("common.done")} />;
  if (!start) return err ? <div className="box bad">{err}</div> : <p className="muted">{t("tf.preparing")}</p>;

  return (
    <form onSubmit={confirm} className="tf-setup">
      <div className="tf-grid">
        <div className="qr" aria-label={t("tf.qrAria")} dangerouslySetInnerHTML={{ __html: start.qr_svg ?? "" }} />
        <div>
          <ol className="tf-steps">
            <li>{t("tf.step1")}</li>
            <li>{t("tf.step2")}</li>
            <li>{t("tf.step3")}</li>
          </ol>
          <details className="tf-manual">
            <summary>{t("tf.cantScan")}</summary>
            <p className="small-text muted">{t("tf.manual")}</p>
            <div className="secret">
              <code>{group(start.secret)}</code>
              <button type="button" className="ghost small" onClick={() => copy(start.secret)}>
                {t("common.copy")}
              </button>
            </div>
          </details>
        </div>
      </div>
      <CodeField value={code} onChange={setCode} autoFocus />
      {err && <div className="box bad">{err}</div>}
      <div className="nav">
        {props.onCancel ? (
          <button type="button" className="ghost" onClick={props.onCancel}>
            {t("common.cancel")}
          </button>
        ) : (
          <span />
        )}
        <button className="primary" disabled={busy || code.replace(/\s/g, "").length !== 6}>
          <IconCheck /> {busy ? t("tf.verifying") : t("tf.confirm")}
        </button>
      </div>
    </form>
  );
}
