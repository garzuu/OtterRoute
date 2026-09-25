import { useState, type FormEvent } from "react";
import { api } from "./api";
import { TwoFactorSetup } from "./TwoFactor";

function Shell(props: { title: string; lead: string; onLogout: () => void; children: React.ReactNode }) {
  return (
    <div className="shell">
      <div className="card">
        <h1>{props.title}</h1>
        <p className="lead">{props.lead}</p>
        {props.children}
        <div className="mt">
          <button className="link" onClick={props.onLogout}>
            Esci
          </button>
        </div>
      </div>
    </div>
  );
}

/** Password temporanea: va sostituita prima di entrare. */
export function ForcedPassword(props: { onDone: () => void; onLogout: () => void }) {
  const [cur, setCur] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const valid = cur && next.length >= 10 && next === again;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      await api.changePassword(cur, next);
      props.onDone();
    } catch (x) {
      setErr((x as Error).message);
      setBusy(false);
    }
  };

  return (
    <Shell
      title="Scegli una nuova password"
      lead="Stai usando una password temporanea. Sceglierne una tua è necessario per continuare."
      onLogout={props.onLogout}
    >
      <form onSubmit={submit}>
        <label className="field">
          <span className="flabel">Password attuale (temporanea)</span>
          <input type="password" value={cur} onChange={(e) => setCur(e.target.value)} autoComplete="current-password" autoFocus />
        </label>
        <label className="field">
          <span className="flabel">Nuova password</span>
          <input type="password" value={next} onChange={(e) => setNext(e.target.value)} autoComplete="new-password" />
          <span className="hint">Almeno 10 caratteri.</span>
        </label>
        <label className="field">
          <span className="flabel">Ripeti la nuova password</span>
          <input type="password" value={again} onChange={(e) => setAgain(e.target.value)} autoComplete="new-password" />
          {again && next !== again && <span className="hint bad-text">Le password non coincidono.</span>}
        </label>
        {err && <div className="box bad">{err}</div>}
        <div className="nav">
          <span />
          <button className="primary" disabled={!valid || busy}>
            {busy ? "Attendi…" : "Cambia password"}
          </button>
        </div>
      </form>
    </Shell>
  );
}

/** Il criterio di sicurezza richiede la 2FA: va attivata prima di entrare. */
export function ForcedTwoFactor(props: { onDone: () => void; onLogout: () => void }) {
  return (
    <Shell
      title="Attiva la verifica in due passaggi"
      lead="Il criterio di sicurezza di questo nodo richiede un secondo fattore per il tuo account."
      onLogout={props.onLogout}
    >
      <TwoFactorSetup onDone={props.onDone} />
    </Shell>
  );
}
