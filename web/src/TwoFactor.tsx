import { useEffect, useState, type FormEvent } from "react";
import { api, type TwoFactorStart } from "./api";
import { IconCheck } from "./icons";

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
      aria-label="Codice a 6 cifre"
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
  const [saved, setSaved] = useState(false);
  const text = `OtterRoute · codici di recupero\n\n${props.codes.join("\n")}\n`;
  return (
    <div>
      <div className="box warn">
        <strong>Conservali adesso.</strong> Non verranno mostrati di nuovo. Ognuno vale una volta sola e serve se perdi
        il telefono.
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
          Copia
        </button>
        <button className="secondary small" onClick={() => download("otterroute-codici-di-recupero.txt", text)}>
          Scarica
        </button>
      </div>
      <label className="check mt">
        <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        <span>Ho salvato i codici in un posto sicuro.</span>
      </label>
      <div className="nav">
        <span />
        <button className="primary" disabled={!saved} onClick={props.onDone}>
          {props.doneLabel ?? "Fine"}
        </button>
      </div>
    </div>
  );
}

/** Attivazione della 2FA: QR → codice di conferma → codici di recupero. */
export function TwoFactorSetup(props: { onDone: () => void; onCancel?: () => void }) {
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

  if (codes) return <RecoveryCodes codes={codes} onDone={props.onDone} doneLabel="Fine" />;
  if (!start) return err ? <div className="box bad">{err}</div> : <p className="muted">Preparo il codice…</p>;

  return (
    <form onSubmit={confirm} className="tf-setup">
      <div className="tf-grid">
        <div className="qr" aria-label="Codice QR" dangerouslySetInnerHTML={{ __html: start.qr_svg ?? "" }} />
        <div>
          <ol className="tf-steps">
            <li>Apri la tua app di autenticazione (Google Authenticator, 1Password, Authy…).</li>
            <li>Scansiona il codice QR.</li>
            <li>Scrivi qui il codice a 6 cifre che l’app mostra.</li>
          </ol>
          <details className="tf-manual">
            <summary>Non riesci a scansionare?</summary>
            <p className="small-text muted">Aggiungi un account a mano con questa chiave:</p>
            <div className="secret">
              <code>{group(start.secret)}</code>
              <button type="button" className="ghost small" onClick={() => copy(start.secret)}>
                Copia
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
            Annulla
          </button>
        ) : (
          <span />
        )}
        <button className="primary" disabled={busy || code.replace(/\s/g, "").length !== 6}>
          <IconCheck /> {busy ? "Verifico…" : "Conferma e attiva"}
        </button>
      </div>
    </form>
  );
}
