import { useEffect, useState, type FormEvent } from "react";
import { api, type TwoFactorPolicy, type UserInfo } from "../api";
import { ROLE_HELP, ROLE_LABEL, useAuth } from "../auth";
import { RecoveryCodes, TwoFactorSetup, CodeField } from "../TwoFactor";
import { useT } from "../i18n";
import { Modal, Page, fmtTime } from "../ui";

/** Il mio profilo: dati, password e verifica in due passaggi. */
export function Profile() {
  const t = useT();
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
    <Page title={t("profile.title")} lead={t("profile.lead")}>
      <div className="card">
        <h2>{t("profile.account")}</h2>
        <dl className="kv">
          <dt>{t("profile.user")}</dt>
          <dd>
            <strong>{user.username}</strong>
          </dd>
          <dt>{t("profile.role")}</dt>
          <dd>
            {ROLE_LABEL[user.role] ?? user.role}
            <div className="muted small-text">{ROLE_HELP[user.role]}</div>
          </dd>
          <dt>{t("profile.scopes")}</dt>
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
              <dt>{t("profile.lastLogin")}</dt>
              <dd>{fmtTime(me.last_login_at)}</dd>
            </>
          )}
        </dl>
      </div>

      <PasswordCard />

      <div className="card">
        <div className="head">
          <h2>{t("tfa.title")}</h2>
          <span className={user.two_factor ? "badge good" : "badge"}>{user.two_factor ? t("tfa.on") : t("tfa.off")}</span>
        </div>
        <p className="lead small-lead">
          {t("tfa.lead")}
        </p>
        {required && !user.two_factor && <div className="box warn">{t("tfa.policy")}</div>}
        <div className="actions">
          {!user.two_factor ? (
            <button className="primary" onClick={() => setModal("setup")}>
              {t("tfa.enable")}
            </button>
          ) : (
            <>
              <button className="secondary" onClick={() => setModal("recovery")}>
                {t("tfa.regen")}
              </button>
              {required ? (
                <span className="muted small-text">{t("tfa.locked")}</span>
              ) : (
                <button className="ghost" onClick={() => setModal("disable")}>
                  {t("common.disable")}
                </button>
              )}
            </>
          )}
        </div>
      </div>

      {modal === "setup" && (
        <Modal title={t("tfa.setupTitle")} onClose={() => setModal(null)} wide>
          <TwoFactorSetup onDone={done} onCancel={() => setModal(null)} />
        </Modal>
      )}
      {modal === "recovery" && <RecoveryModal onClose={() => setModal(null)} />}
      {modal === "disable" && <DisableModal onClose={() => setModal(null)} onDone={done} />}
    </Page>
  );
}

function PasswordCard() {
  const t = useT();
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
      setMsg({ ok: true, text: t("pw.changed") });
    } catch (x) {
      setMsg({ ok: false, text: (x as Error).message });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card">
      <h2>{t("pw.title")}</h2>
      <form onSubmit={submit}>
        <div className="row">
          <label className="field">
            <span className="flabel">{t("pw.current")}</span>
            <input type="password" value={cur} onChange={(e) => setCur(e.target.value)} autoComplete="current-password" />
          </label>
          <span />
        </div>
        <div className="row">
          <label className="field">
            <span className="flabel">{t("pw.new")}</span>
            <input type="password" value={next} onChange={(e) => setNext(e.target.value)} autoComplete="new-password" />
            <span className="hint">{t("pw.min")}</span>
          </label>
          <label className="field">
            <span className="flabel">{t("pw.again")}</span>
            <input type="password" value={again} onChange={(e) => setAgain(e.target.value)} autoComplete="new-password" />
            {again && next !== again && <span className="hint bad-text">{t("pw.mismatch")}</span>}
          </label>
        </div>
        {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
        <div className="nav">
          <span />
          <button className="primary" disabled={!valid || busy}>
            {busy ? t("common.wait") : t("pw.change")}
          </button>
        </div>
      </form>
    </div>
  );
}

function RecoveryModal(props: { onClose: () => void }) {
  const t = useT();
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
    <Modal title={t("tfa.recoveryTitle")} onClose={props.onClose}>
      {codes ? (
        <RecoveryCodes codes={codes} onDone={props.onClose} />
      ) : (
        <form onSubmit={submit}>
          <p className="lead small-lead">
            {t("tfa.recoveryLead")}
          </p>
          <label className="field">
            <span className="flabel">{t("common.password")}</span>
            <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" autoFocus />
          </label>
          {err && <div className="box bad">{err}</div>}
          <div className="nav">
            <button type="button" className="ghost" onClick={props.onClose}>
              {t("common.cancel")}
            </button>
            <button className="primary" disabled={!password || busy}>
              {busy ? t("common.wait") : t("tfa.recoveryGo")}
            </button>
          </div>
        </form>
      )}
    </Modal>
  );
}

function DisableModal(props: { onClose: () => void; onDone: () => void }) {
  const t = useT();
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
    <Modal title={t("tfa.disableTitle")} onClose={props.onClose}>
      <form onSubmit={submit}>
        <p className="lead small-lead">{t("tfa.disableLead")}</p>
        <label className="field">
          <span className="flabel">{t("common.password")}</span>
          <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" autoFocus />
        </label>
        <CodeField value={code} onChange={setCode} />
        {err && <div className="box bad">{err}</div>}
        <div className="nav">
          <button type="button" className="ghost" onClick={props.onClose}>
            {t("common.cancel")}
          </button>
          <button className="danger" disabled={!password || code.replace(/\s/g, "").length !== 6 || busy}>
            {busy ? t("common.wait") : t("common.disable")}
          </button>
        </div>
      </form>
    </Modal>
  );
}
