import { useEffect, useState } from "react";
import { api, type NotifyConfig, type NotifyInfo, type NotifyLogEntry, type Threshold } from "../api";
import { DataTable, type Column } from "../DataTable";
import { Field, Page, fmtTime } from "../ui";

const KIND_LABEL: Record<NotifyLogEntry["kind"], string> = {
  problem: "Problema",
  recovered: "Ripristinato",
  reminder: "Promemoria",
  test: "Prova",
};

const lines = (v: string) =>
  v
    .split(/[\n,;]+/)
    .map((x) => x.trim())
    .filter(Boolean);

function ThresholdSelect(props: { value: Threshold; onChange: (t: Threshold) => void }) {
  return (
    <select value={props.value} onChange={(e) => props.onChange(e.target.value as Threshold)}>
      <option value="errors">Solo errori</option>
      <option value="warnings">Avvisi ed errori</option>
    </select>
  );
}

export function Notifications() {
  const [info, setInfo] = useState<NotifyInfo | null>(null);
  const [cfg, setCfg] = useState<NotifyConfig | null>(null);
  const [smtpPw, setSmtpPw] = useState("");
  const [token, setToken] = useState("");
  const [to, setTo] = useState("");
  const [chats, setChats] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState<string | null>(null);
  const [log, setLog] = useState<NotifyLogEntry[]>([]);

  const apply = (i: NotifyInfo) => {
    setInfo(i);
    setCfg(i.config);
    setTo(i.config.email.to.join("\n"));
    setChats(i.config.telegram.chat_ids.join("\n"));
    setSmtpPw("");
    setToken("");
    setLog(i.log);
  };

  useEffect(() => {
    api.notifications().then(apply).catch((e: Error) => setMsg({ ok: false, text: e.message }));
  }, []);

  if (!info || !cfg)
    return (
      <Page title="Notifiche">
        {msg ? <div className="box bad">{msg.text}</div> : <p className="muted">Carico…</p>}
      </Page>
    );

  const full = (): NotifyConfig => ({
    ...cfg,
    email: { ...cfg.email, to: lines(to) },
    telegram: { ...cfg.telegram, chat_ids: lines(chats) },
  });
  const dirty = JSON.stringify(full()) !== JSON.stringify(info.config) || smtpPw !== "" || token !== "";

  const setEmail = (p: Partial<NotifyConfig["email"]>) => setCfg({ ...cfg, email: { ...cfg.email, ...p } });
  const setTg = (p: Partial<NotifyConfig["telegram"]>) => setCfg({ ...cfg, telegram: { ...cfg.telegram, ...p } });
  const num = (v: string) => Number(v.replace(/\D/g, "") || 0);

  const save = async () => {
    setBusy(true);
    setMsg(null);
    try {
      apply(await api.saveNotifications({ config: full(), smtp_password: smtpPw || undefined, telegram_token: token || undefined }));
      setMsg({ ok: true, text: "Impostazioni salvate." });
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  const test = async (ch: "email" | "telegram") => {
    setTesting(ch);
    setMsg(null);
    try {
      const r = await api.testNotification(ch);
      setLog(r.log);
      setMsg(r.ok ? { ok: true, text: "Messaggio di prova inviato: controlla la ricezione." } : { ok: false, text: `Invio non riuscito: ${r.error}` });
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setTesting(null);
    }
  };

  const columns: Column<NotifyLogEntry>[] = [
    { key: "at", header: "Quando", render: (l) => fmtTime(l.at), sort: (l) => l.at, width: "170px" },
    { key: "channel", header: "Canale", render: (l) => (l.channel === "email" ? "Email" : "Telegram"), sort: (l) => l.channel },
    { key: "kind", header: "Tipo", render: (l) => KIND_LABEL[l.kind], sort: (l) => l.kind },
    { key: "title", header: "Messaggio", render: (l) => l.title },
    {
      key: "ok",
      header: "Esito",
      render: (l) => (l.ok ? <span className="badge good">Inviato</span> : <span className="badge bad" title={l.error ?? ""}>Non recapitato</span>),
      sort: (l) => (l.ok ? 1 : 0),
    },
  ];

  const enabledAny = cfg.email.enabled || cfg.telegram.enabled;

  return (
    <Page
      title="Notifiche"
      lead="Ricevi un messaggio quando un dominio o un bucket smette di funzionare, e quando torna a posto. Gli eventi sono gli stessi avvisi della campanella."
    >
      {!enabledAny && <div className="box">Nessun canale attivo: attiva l’email o Telegram per iniziare a ricevere le notifiche.</div>}
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}

      <div className="card">
        <h2>Email</h2>
        <label className="check">
          <input type="checkbox" checked={cfg.email.enabled} onChange={(e) => setEmail({ enabled: e.target.checked })} />
          Invia le notifiche via email (SMTP)
        </label>
        <div className="row">
          <Field label="Server SMTP" hint="Solo il nome, es. smtp.example.com.">
            <input value={cfg.email.host} onChange={(e) => setEmail({ host: e.target.value.trim() })} placeholder="smtp.example.com" />
          </Field>
          <Field label="Porta">
            <input value={String(cfg.email.port)} onChange={(e) => setEmail({ port: num(e.target.value) })} inputMode="numeric" />
          </Field>
          <Field label="Sicurezza" hint="STARTTLS di solito con 587, TLS con 465.">
            <select value={cfg.email.security} onChange={(e) => setEmail({ security: e.target.value as NotifyConfig["email"]["security"] })}>
              <option value="starttls">STARTTLS</option>
              <option value="tls">TLS</option>
              <option value="none">Nessuna (solo relay locale)</option>
            </select>
          </Field>
        </div>
        <div className="row">
          <Field label="Utente" hint="Lascia vuoto se il server non richiede l’accesso.">
            <input value={cfg.email.user} onChange={(e) => setEmail({ user: e.target.value })} autoComplete="off" />
          </Field>
          <Field label="Password" hint={info.has_smtp_password ? "Salvata: lascia vuoto per non cambiarla." : undefined}>
            <input type="password" value={smtpPw} onChange={(e) => setSmtpPw(e.target.value)} placeholder={info.has_smtp_password ? "••••••••" : ""} autoComplete="new-password" />
          </Field>
        </div>
        <div className="row">
          <Field label="Mittente" hint="Es. OtterRoute <otterroute@example.com>">
            <input value={cfg.email.from} onChange={(e) => setEmail({ from: e.target.value })} placeholder="otterroute@example.com" />
          </Field>
          <Field label="Notifica">
            <ThresholdSelect value={cfg.email.threshold} onChange={(threshold) => setEmail({ threshold })} />
          </Field>
        </div>
        <Field label="Destinatari" hint="Uno per riga (massimo 10).">
          <textarea rows={3} value={to} onChange={(e) => setTo(e.target.value)} placeholder="nome@example.com" />
        </Field>
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => test("email")} disabled={dirty || testing !== null} title={dirty ? "Salva prima le impostazioni" : undefined}>
            {testing === "email" ? "Invio…" : "Invia prova"}
          </button>
        </div>
      </div>

      <div className="card">
        <h2>Telegram</h2>
        <label className="check">
          <input type="checkbox" checked={cfg.telegram.enabled} onChange={(e) => setTg({ enabled: e.target.checked })} />
          Invia le notifiche su Telegram
        </label>
        <div className="row">
          <Field label="Token del bot" hint={info.has_telegram_token ? "Salvato: lascia vuoto per non cambiarlo." : "Lo ottieni da @BotFather."}>
            <input type="password" value={token} onChange={(e) => setToken(e.target.value)} placeholder={info.has_telegram_token ? "••••••••" : "123456:ABC…"} autoComplete="new-password" />
          </Field>
          <Field label="Notifica">
            <ThresholdSelect value={cfg.telegram.threshold} onChange={(threshold) => setTg({ threshold })} />
          </Field>
        </div>
        <Field label="Chat" hint="Uno per riga: l’id numerico di una chat o di un gruppo (es. -1001234567890) oppure @nomecanale. Il bot deve poter scrivere lì.">
          <textarea rows={3} value={chats} onChange={(e) => setChats(e.target.value)} placeholder="123456789" />
        </Field>
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => test("telegram")} disabled={dirty || testing !== null} title={dirty ? "Salva prima le impostazioni" : undefined}>
            {testing === "telegram" ? "Invio…" : "Invia prova"}
          </button>
        </div>
      </div>

      <div className="card">
        <h2>Regole</h2>
        <div className="row">
          <Field label="Attesa prima di notificare (minuti)" hint="Un problema che rientra prima non genera messaggi. 0 = subito.">
            <input value={String(cfg.delay_min)} onChange={(e) => setCfg({ ...cfg, delay_min: num(e.target.value) })} inputMode="numeric" />
          </Field>
          <Field label="Promemoria ogni (ore)" hint="Ripete il messaggio finché il problema resta aperto. 0 = mai.">
            <input value={String(cfg.reminder_hours)} onChange={(e) => setCfg({ ...cfg, reminder_hours: num(e.target.value) })} inputMode="numeric" />
          </Field>
        </div>
        <label className="check">
          <input type="checkbox" checked={cfg.recovery} onChange={(e) => setCfg({ ...cfg, recovery: e.target.checked })} />
          Avvisa anche quando il problema rientra
        </label>
        <div className="nav">
          <span />
          <button className="primary" onClick={save} disabled={busy || !dirty}>
            {busy ? "Salvo…" : "Salva"}
          </button>
        </div>
      </div>

      <div className="card">
        <h2>Ultime notifiche</h2>
        <DataTable
          columns={columns}
          rows={log}
          rowKey={(l) => `${l.at}${l.channel}${l.kind}${l.title}`}
          empty="Ancora nessuna notifica inviata."
          initialSort={{ key: "at", dir: "desc" }}
          expand={(l) => (l.error ? <div className="box bad">{l.error}</div> : null)}
        />
      </div>
    </Page>
  );
}
