import { useState, type FormEvent, type ReactNode } from "react";
import { api } from "./api";
import { CodeField } from "./TwoFactor";
import { IconCheck, IconEye, IconEyeOff, IconLock, IconUser } from "./icons";
import { LangToggle, tr, useT, type Key } from "./i18n";
import { Illus } from "./ui";

const POINTS: Key[] = ["auth.point1", "auth.point2", "auth.point3"];

/** Robustezza indicativa: lunghezza e varietà di caratteri. */
function strength(pw: string): { score: number; label: string } {
  if (!pw) return { score: 0, label: "" };
  let score = pw.length >= 10 ? 1 : 0;
  if (pw.length >= 14) score++;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw) && /\d/.test(pw)) score++;
  if (/[^A-Za-z0-9]/.test(pw)) score++;
  return { score, label: tr(`auth.str${score}` as Key) };
}

function Input(props: {
  label: string;
  icon: ReactNode;
  type?: "text" | "password";
  value: string;
  onChange: (v: string) => void;
  autoComplete: string;
  autoFocus?: boolean;
  onCaps?: (on: boolean) => void;
}) {
  const [shown, setShown] = useState(false);
  const isPw = props.type === "password";
  return (
    <label className="afield">
      <span className="flabel">{props.label}</span>
      <span className="ainput">
        <span className="aicon" aria-hidden>
          {props.icon}
        </span>
        <input
          type={isPw && !shown ? "password" : "text"}
          value={props.value}
          onChange={(e) => props.onChange(e.target.value)}
          onKeyUp={(e) => props.onCaps?.(e.getModifierState("CapsLock"))}
          autoComplete={props.autoComplete}
          autoFocus={props.autoFocus}
          spellCheck={false}
          autoCapitalize="off"
        />
        {isPw && (
          <button
            type="button"
            className="aeye"
            onClick={() => setShown(!shown)}
            aria-label={shown ? tr("auth.hidePw") : tr("auth.showPw")}
            tabIndex={-1}
          >
            {shown ? <IconEyeOff /> : <IconEye />}
          </button>
        )}
      </span>
    </label>
  );
}

export function AuthScreen(props: { mode: "setup" | "login"; onDone: () => void }) {
  const t = useT();
  const setup = props.mode === "setup";
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [caps, setCaps] = useState(false);
  // secondo passo: la password è giusta ma serve il codice dell'app di autenticazione
  const [challenge, setChallenge] = useState<string | null>(null);
  const [code, setCode] = useState("");
  const [useRecovery, setUseRecovery] = useState(false);

  const st = strength(password);
  const mismatch = setup && confirm !== "" && confirm !== password;
  const valid = username.trim().length >= 3 && password.length >= (setup ? 10 : 1) && (!setup || confirm === password);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      if (setup) {
        await api.setup(username, password);
      } else {
        const r = await api.login(username, password);
        if ("needs_2fa" in r) {
          setChallenge(r.challenge);
          setBusy(false);
          return;
        }
      }
      props.onDone();
    } catch (e) {
      setErr((e as Error).message);
      setBusy(false);
    }
  };

  const submitCode = async (e: FormEvent) => {
    e.preventDefault();
    if (!challenge) return;
    setBusy(true);
    setErr(null);
    try {
      await api.login2fa(challenge, useRecovery ? { recovery_code: code } : { code });
      props.onDone();
    } catch (x) {
      setErr((x as Error).message);
      setBusy(false);
    }
  };

  const secondStep = challenge && (
    <form className="auth-form" onSubmit={submitCode}>
      <div className="auth-mobile-brand">
        <img className="logo-img" src="/logo-tile.png" alt="" width={44} height={44} />
        <span>OtterRoute</span>
      </div>
      <h1>{t("auth.2fa.title")}</h1>
      <p className="lead">
        {useRecovery
          ? t("auth.2fa.recoveryLead")
          : t("auth.2fa.appLead", { user: username })}
      </p>
      {useRecovery ? (
        <input
          className="codefield"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="xxxxx-xxxxx"
          autoFocus
          spellCheck={false}
          aria-label={t("auth.2fa.recoveryAria")}
        />
      ) : (
        <CodeField value={code} onChange={setCode} autoFocus />
      )}
      {err && <div className="box bad">{err}</div>}
      <button className="primary abtn" disabled={busy || code.replace(/\s/g, "").length < 6}>
        {busy ? t("common.wait") : t("auth.2fa.verify")}
      </button>
      <div className="auth-alt">
        <button
          type="button"
          className="link"
          onClick={() => {
            setUseRecovery(!useRecovery);
            setCode("");
            setErr(null);
          }}
        >
          {useRecovery ? t("auth.2fa.useApp") : t("auth.2fa.useRecovery")}
        </button>
        <button
          type="button"
          className="link"
          onClick={() => {
            setChallenge(null);
            setCode("");
            setErr(null);
            setPassword("");
          }}
        >
          {t("auth.back")}
        </button>
      </div>
    </form>
  );

  return (
    <div className="auth">
      <aside className="auth-brand" aria-hidden={false}>
        <div className="auth-brand-top">
          <img className="logo-img" src="/logo-tile.png" alt="" width={40} height={40} />
          <span>OtterRoute</span>
        </div>

        <div className="auth-hero">
          <div className="auth-stage">
            <span className="ring r1" aria-hidden />
            <span className="ring r2" aria-hidden />
            <Illus name={setup ? "welcome" : "laptop"} width={setup ? 230 : 280} />
          </div>
          <h2>
            {t("auth.hero1")}
            <br />
            {t("auth.hero2")}
          </h2>
          <p>{t("auth.heroText")}</p>
          <ul>
            {POINTS.map((k) => (
              <li key={k}>
                <span className="tick" aria-hidden>
                  <IconCheck />
                </span>
                {t(k)}
              </li>
            ))}
          </ul>
        </div>

        <div className="auth-brand-foot">{t("auth.foot")}</div>
      </aside>

      <main className="auth-form-wrap">
        <div style={{ position: "absolute", top: 16, right: 20 }}>
          <LangToggle />
        </div>
        {secondStep || (
        <form className="auth-form" onSubmit={submit}>
          <div className="auth-mobile-brand">
            <img className="logo-img" src="/logo-tile.png" alt="" width={44} height={44} />
            <span>OtterRoute</span>
          </div>

          <h1>{setup ? t("auth.setupTitle") : t("auth.loginTitle")}</h1>
          <p className="lead">
            {setup ? t("auth.setupLead") : t("auth.loginLead")}
          </p>

          <Input label={t("auth.username")} icon={<IconUser />} value={username} onChange={setUsername} autoComplete="username" autoFocus />
          <Input
            label={t("common.password")}
            icon={<IconLock />}
            type="password"
            value={password}
            onChange={setPassword}
            autoComplete={setup ? "new-password" : "current-password"}
            onCaps={setCaps}
          />
          {setup && (
            <div className="meter" aria-live="polite">
              <div className="bars" aria-hidden>
                {[1, 2, 3, 4].map((i) => (
                  <span key={i} className={i <= st.score ? `on s${st.score}` : ""} />
                ))}
              </div>
              <span className="muted small-text">{password ? st.label : t("auth.min10")}</span>
            </div>
          )}
          {setup && (
            <Input
              label={t("auth.repeat")}
              icon={<IconLock />}
              type="password"
              value={confirm}
              onChange={setConfirm}
              autoComplete="new-password"
              onCaps={setCaps}
            />
          )}
          {mismatch && <p className="ahint bad-text">{t("forced.pw.mismatch")}</p>}
          {caps && <p className="ahint warn-text">{t("auth.caps")}</p>}
          {err && <div className="box bad">{err}</div>}

          <button className="primary abtn" disabled={!valid || busy}>
            {busy ? t("common.wait") : setup ? t("auth.create") : t("auth.login")}
          </button>
        </form>
        )}
      </main>
    </div>
  );
}
