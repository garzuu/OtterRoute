import { useState, type FormEvent, type ReactNode } from "react";
import { api } from "./api";
import { CodeField } from "./TwoFactor";
import { IconCheck, IconEye, IconEyeOff, IconLock, IconUser } from "./icons";
import { Illus } from "./ui";

const POINTS = [
  "Più domini e più bucket S3, un solo nodo",
  "Cache su disco con svuotamento immediato",
  "Controlli automatici di DNS e storage",
];

/** Robustezza indicativa: lunghezza e varietà di caratteri. */
function strength(pw: string): { score: number; label: string } {
  if (!pw) return { score: 0, label: "" };
  let score = pw.length >= 10 ? 1 : 0;
  if (pw.length >= 14) score++;
  if (/[a-z]/.test(pw) && /[A-Z]/.test(pw) && /\d/.test(pw)) score++;
  if (/[^A-Za-z0-9]/.test(pw)) score++;
  return { score, label: ["Troppo corta", "Debole", "Discreta", "Buona", "Ottima"][score] };
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
            aria-label={shown ? "Nascondi la password" : "Mostra la password"}
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
      <h1>Verifica in due passaggi</h1>
      <p className="lead">
        {useRecovery
          ? "Inserisci uno dei codici di recupero che hai salvato. Ognuno vale una volta sola."
          : "Apri l’app di autenticazione e inserisci il codice a 6 cifre per "}
        {!useRecovery && <strong>{username}</strong>}
        {!useRecovery && "."}
      </p>
      {useRecovery ? (
        <input
          className="codefield"
          value={code}
          onChange={(e) => setCode(e.target.value)}
          placeholder="xxxxx-xxxxx"
          autoFocus
          spellCheck={false}
          aria-label="Codice di recupero"
        />
      ) : (
        <CodeField value={code} onChange={setCode} autoFocus />
      )}
      {err && <div className="box bad">{err}</div>}
      <button className="primary abtn" disabled={busy || code.replace(/\s/g, "").length < 6}>
        {busy ? "Attendi…" : "Verifica"}
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
          {useRecovery ? "Usa il codice dell’app" : "Usa un codice di recupero"}
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
          Indietro
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
          <h2>Pubblica i tuoi file,
            <br />
            ovunque.</h2>
          <p>Un gateway self-hosted per i tuoi bucket S3: più domini, cache su disco e nessuna configurazione di proxy.</p>
          <ul>
            {POINTS.map((t) => (
              <li key={t}>
                <span className="tick" aria-hidden>
                  <IconCheck />
                </span>
                {t}
              </li>
            ))}
          </ul>
        </div>

        <div className="auth-brand-foot">Gateway per storage S3 · self-hosted</div>
      </aside>

      <main className="auth-form-wrap">
        {secondStep || (
        <form className="auth-form" onSubmit={submit}>
          <div className="auth-mobile-brand">
            <img className="logo-img" src="/logo-tile.png" alt="" width={44} height={44} />
            <span>OtterRoute</span>
          </div>

          <h1>{setup ? "Benvenuto! Crea l’amministratore" : "Bentornato"}</h1>
          <p className="lead">
            {setup
              ? "È il primo avvio di questo nodo. Scegli le credenziali con cui gestirai domini, bucket e instradamenti."
              : "Accedi al pannello per gestire domini, bucket e instradamenti."}
          </p>

          <Input label="Nome utente" icon={<IconUser />} value={username} onChange={setUsername} autoComplete="username" autoFocus />
          <Input
            label="Password"
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
              <span className="muted small-text">{password ? st.label : "Almeno 10 caratteri"}</span>
            </div>
          )}
          {setup && (
            <Input
              label="Ripeti la password"
              icon={<IconLock />}
              type="password"
              value={confirm}
              onChange={setConfirm}
              autoComplete="new-password"
              onCaps={setCaps}
            />
          )}
          {mismatch && <p className="ahint bad-text">Le password non coincidono.</p>}
          {caps && <p className="ahint warn-text">Bloc Maiusc attivo.</p>}
          {err && <div className="box bad">{err}</div>}

          <button className="primary abtn" disabled={!valid || busy}>
            {busy ? "Attendi…" : setup ? "Crea e continua" : "Accedi"}
          </button>
        </form>
        )}
      </main>
    </div>
  );
}
