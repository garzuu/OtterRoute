import { useEffect, useState, type FormEvent } from "react";
import { api, type TwoFactorPolicy, type UserInfo } from "../api";
import { ROLE_HELP, ROLE_LABEL, useAuth } from "../auth";
import { RecoveryCodes, TwoFactorSetup, CodeField } from "../TwoFactor";
import { Modal, Page, fmtTime } from "../ui";

/** Il mio profilo: dati, password e verifica in due passaggi. */
export function Profile() {
  const { user, reloadSession } = useAuth();
  const [me, setMe] = useState<UserInfo | null>(null);
  const [policy, setPolicy] = useState<TwoFactorPolicy>("off");
  const [modal, setModal] = useState<"setup" | "disable" | "recovery" | null>(null);

  const load = () =>
    api
      .me()
      .then((r) => {
        setMe(r.user);
        setPolicy(r.policy.require_2fa);
      })
      .catch(() => undefined);
  useEffect(() => {
    void load();
  }, []);

  const done = () => {
    setModal(null);
    void load();
    reloadSession();
  };

  const required = policy === "all" || (policy === "managers" && user.scopes.includes("users:manage"));

  return (
    <Page title="Il mio profilo" lead="Le tue credenziali e la sicurezza del tuo account.">
      <div className="card">
        <h2>Account</h2>
        <dl className="kv">
          <dt>Utente</dt>
          <dd>
            <strong>{user.username}</strong>
          </dd>
          <dt>Ruolo</dt>
          <dd>
            {ROLE_LABEL[user.role] ?? user.role}
            <div className="muted small-text">{ROLE_HELP[user.role]}</div>
          </dd>
          <dt>Permessi</dt>
          <dd>
            <div className="chips">
              {user.scopes.map((s) => (
                <code key={s} className="chip">
                  {s}
                </code>
              ))}
            </div>
          </dd>
          {me && (
            <>
              <dt>Ultimo accesso</dt>
              <dd>{fmtTime(me.last_login_at)}</dd>
            </>
          )}
        </dl>
      </div>

      <PasswordCard />

      <div className="card">
        <div className="head">
          <h2>Verifica in due passaggi</h2>
          <span className={user.two_factor ? "badge good" : "badge"}>{user.two_factor ? "Attiva" : "Non attiva"}</span>
        </div>
        <p className="lead small-lead">
          Oltre alla password serve un codice a 6 cifre generato da un’app di autenticazione sul tuo telefono. Anche chi
          scoprisse la password non potrebbe entrare.
        </p>
        {required && !user.two_factor && <div className="box warn">Il criterio di sicurezza richiede la 2FA per il tuo account.</div>}
        <div className="actions">
          {!user.two_factor ? (
            <button className="primary" onClick={() => setModal("setup")}>
              Attiva la 2FA
            </button>
          ) : (
            <>
              <button className="secondary" onClick={() => setModal("recovery")}>
                Rigenera i codici di recupero
              </button>
              {required ? (
                <span className="muted small-text">Non si può disattivare: il criterio la richiede.</span>
              ) : (
                <button className="ghost" onClick={() => setModal("disable")}>
                  Disattiva
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {modal === "setup" && (
        <Modal title="Attiva la verifica in due passaggi" onClose={() => setModal(null)} wide>
          <TwoFactorSetup onDone={done} onCancel={() => setModal(null)} />
        </Modal>
      )}
      {modal === "recovery" && <RecoveryModal onClose={() => setModal(null)} />}
      {modal === "disable" && <DisableModal onClose={() => setModal(null)} onDone={done} />}
    </Page>
  );
}

function PasswordCard() {
  const [cur, setCur] = useState("");
  const [next, setNext] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const valid = cur && next.length >= 10 && next === again;

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    try {
      await api.changePassword(cur, next);
      setCur("");
      setNext("");
      setAgain("");
      setMsg({ ok: true, text: "Password cambiata. Le tue altre sessioni sono state chiuse." });
    } catch (x) {
      setMsg({ ok: false, text: (x as Error).message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card">
      <h2>Password</h2>
      <form onSubmit={submit}>
        <div className="row">
          <label className="field">
            <span className="flabel">Password attuale</span>
            <input type="password" value={cur} onChange={(e) => setCur(e.target.value)} autoComplete="current-password" />
          </label>
          <span />
        </div>
        <div className="row">
          <label className="field">
            <span className="flabel">Nuova password</span>
            <input type="password" value={next} onChange={(e) => setNext(e.target.value)} autoComplete="new-password" />
            <span className="hint">Almeno 10 caratteri.</span>
          </label>
          <label className="field">
            <span className="flabel">Ripeti la nuova password</span>
            <input type="password" value={again} onChange={(e) => setAgain(e.target.value)} autoComplete="new-password" />
            {again && next !== again && <span className="hint bad-text">Non coincidono.</span>}
          </label>
        </div>
        {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
        <div className="nav">
          <span />
          <button className="primary" disabled={!valid || busy}>
            {busy ? "Attendi…" : "Cambia password"}
          </button>
        </div>
      </form>
    </div>
  );
}

function RecoveryModal(props: { onClose: () => void }) {
  const [password, setPassword] = useState("");
  const [codes, setCodes] = useState<string[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      setCodes((await api.twoFactorRecovery(password)).recovery_codes);
    } catch (x) {
      setErr((x as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal title="Codici di recupero" onClose={props.onClose}>
      {codes ? (
        <RecoveryCodes codes={codes} onDone={props.onClose} />
      ) : (
        <form onSubmit={submit}>
          <p className="lead small-lead">
            Genera dieci codici nuovi: quelli vecchi smettono di funzionare. Conferma con la tua password.
          </p>
          <label className="field">
            <span className="flabel">Password</span>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" autoFocus />
          </label>
          {err && <div className="box bad">{err}</div>}
          <div className="nav">
            <button type="button" className="ghost" onClick={props.onClose}>
              Annulla
            </button>
            <button className="primary" disabled={!password || busy}>
              {busy ? "Attendi…" : "Genera codici nuovi"}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}

function DisableModal(props: { onClose: () => void; onDone: () => void }) {
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      await api.twoFactorDisable(password, { code });
      props.onDone();
    } catch (x) {
      setErr((x as Error).message);
      setBusy(false);
    }
  };

  return (
    <Modal title="Disattiva la 2FA" onClose={props.onClose}>
      <form onSubmit={submit}>
        <p className="lead small-lead">Per sicurezza servono la tua password e un codice valido dell’app.</p>
        <label className="field">
          <span className="flabel">Password</span>
          <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" autoFocus />
        </label>
        <CodeField value={code} onChange={setCode} />
        {err && <div className="box bad">{err}</div>}
        <div className="nav">
          <button type="button" className="ghost" onClick={props.onClose}>
            Annulla
          </button>
          <button className="danger" disabled={!password || code.replace(/\s/g, "").length !== 6 || busy}>
            {busy ? "Attendi…" : "Disattiva"}
          </button>
        </div>
      </form>
    </Modal>
  );
}
