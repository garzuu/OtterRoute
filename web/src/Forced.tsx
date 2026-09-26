import { useState, type FormEvent } from "react";
import { api } from "./api";
import { useT } from "./i18n";
import { TwoFactorSetup } from "./TwoFactor";

function Shell(props: { title: string; lead: string; onLogout: () => void; children: React.ReactNode }) {
  const t = useT();
  return (
    <div className="shell">
      <div className="card">
        <h1>{props.title}</h1>
        <p className="lead">{props.lead}</p>
        {props.children}
        <div className="mt">
          <button className="link" onClick={props.onLogout}>
            {t("menu.logout")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Password temporanea: va sostituita prima di entrare. */
export function ForcedPassword(props: { onDone: () => void; onLogout: () => void }) {
  const t = useT();
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
      title={t("forced.pw.title")}
      lead={t("forced.pw.lead")}
      onLogout={props.onLogout}
    >
      <form onSubmit={submit}>
        <label className="field">
          <span className="flabel">{t("forced.pw.current")}</span>
          <input type="password" value={cur} onChange={(e) => setCur(e.target.value)} autoComplete="current-password" autoFocus />
        </label>
        <label className="field">
          <span className="flabel">{t("pw.new")}</span>
          <input type="password" value={next} onChange={(e) => setNext(e.target.value)} autoComplete="new-password" />
          <span className="hint">{t("pw.min")}</span>
        </label>
        <label className="field">
          <span className="flabel">{t("pw.again")}</span>
          <input type="password" value={again} onChange={(e) => setAgain(e.target.value)} autoComplete="new-password" />
          {again && next !== again && <span className="hint bad-text">{t("forced.pw.mismatch")}</span>}
        </label>
        {err && <div className="box bad">{err}</div>}
        <div className="nav">
          <span />
          <button className="primary" disabled={!valid || busy}>
            {busy ? t("common.wait") : t("pw.change")}
          </button>
        </div>
      </form>
    </Shell>
  );
}

/** Il criterio di sicurezza richiede la 2FA: va attivata prima di entrare. */
export function ForcedTwoFactor(props: { onDone: () => void; onLogout: () => void }) {
  const t = useT();
  return (
    <Shell
      title={t("forced.tfa.title")}
      lead={t("forced.tfa.lead")}
      onLogout={props.onLogout}
    >
      <TwoFactorSetup onDone={props.onDone} />
    </Shell>
  );
}
