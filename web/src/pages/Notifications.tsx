import { useEffect, useState } from "react";
import { api, type NotifyConfig, type NotifyInfo, type NotifyLogEntry, type Threshold } from "../api";
import { DataTable, type Column } from "../DataTable";
import { tr, type Key } from "../i18n";
import { Field, Page, fmtTime } from "../ui";

const KIND_LABEL: Record<NotifyLogEntry["kind"], Key> = {
  problem: "nt.kProblem",
  recovered: "nt.kRecovered",
  reminder: "nt.kReminder",
  test: "nt.kTest",
};

const lines = (v: string) =>
  v
    .split(/[\n,;]+/)
    .map((x) => x.trim())
    .filter(Boolean);

function ThresholdSelect(props: { value: Threshold; onChange: (t: Threshold) => void }) {
  return (
    <select value={props.value} onChange={(e) => props.onChange(e.target.value as Threshold)}>
      <option value="errors">{tr("nt.onlyErrors")}</option>
      <option value="warnings">{tr("nt.warnErrors")}</option>
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
      <Page title={tr("nt.title")}>
        {msg ? <div className="box bad">{msg.text}</div> : <p className="muted">{tr("nt.loading")}</p>}
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
      setMsg({ ok: true, text: tr("nt.saved") });
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
      setMsg(r.ok ? { ok: true, text: tr("nt.testSent") } : { ok: false, text: tr("nt.testFail", { error: r.error ?? "" }) });
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setTesting(null);
    }
  };

  const columns: Column<NotifyLogEntry>[] = [
    { key: "at", header: tr("nt.colWhen"), render: (l) => fmtTime(l.at), sort: (l) => l.at, width: "170px" },
    { key: "channel", header: tr("nt.colChannel"), render: (l) => (l.channel === "email" ? "Email" : "Telegram"), sort: (l) => l.channel },
    { key: "kind", header: tr("nt.colType"), render: (l) => tr(KIND_LABEL[l.kind]), sort: (l) => l.kind },
    { key: "title", header: tr("nt.colMsg"), render: (l) => l.title },
    {
      key: "ok",
      header: tr("nt.colResult"),
      render: (l) => (l.ok ? <span className="badge good">{tr("nt.sent")}</span> : <span className="badge bad" title={l.error ?? ""}>{tr("nt.notDelivered")}</span>),
      sort: (l) => (l.ok ? 1 : 0),
    },
  ];

  const enabledAny = cfg.email.enabled || cfg.telegram.enabled;

  return (
    <Page
      title={tr("nt.title")}
      lead={tr("nt.lead")}
    >
      {!enabledAny && <div className="box">{tr("nt.noChannel")}</div>}
      {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}

      <div className="card">
        <h2>{tr("nt.email")}</h2>
        <label className="check">
          <input type="checkbox" checked={cfg.email.enabled} onChange={(e) => setEmail({ enabled: e.target.checked })} />
          {tr("nt.emailOn")}
        </label>
        <div className="row">
          <Field label={tr("nt.smtpServer")} hint={tr("nt.smtpServerHint")}>
            <input value={cfg.email.host} onChange={(e) => setEmail({ host: e.target.value.trim() })} placeholder="smtp.example.com" />
          </Field>
          <Field label={tr("nt.port")}>
            <input value={String(cfg.email.port)} onChange={(e) => setEmail({ port: num(e.target.value) })} inputMode="numeric" />
          </Field>
          <Field label={tr("nt.security")} hint={tr("nt.securityHint")}>
            <select value={cfg.email.security} onChange={(e) => setEmail({ security: e.target.value as NotifyConfig["email"]["security"] })}>
              <option value="starttls">STARTTLS</option>
              <option value="tls">TLS</option>
              <option value="none">{tr("nt.secNone")}</option>
            </select>
          </Field>
        </div>
        <div className="row">
          <Field label={tr("nt.user")} hint={tr("nt.userHint")}>
            <input value={cfg.email.user} onChange={(e) => setEmail({ user: e.target.value })} autoComplete="off" />
          </Field>
          <Field label={tr("common.password")} hint={info.has_smtp_password ? tr("nt.pwSaved") : undefined}>
            <input type="password" value={smtpPw} onChange={(e) => setSmtpPw(e.target.value)} placeholder={info.has_smtp_password ? "••••••••" : ""} autoComplete="new-password" />
          </Field>
        </div>
        <div className="row">
          <Field label={tr("nt.from")} hint={tr("nt.fromHint")}>
            <input value={cfg.email.from} onChange={(e) => setEmail({ from: e.target.value })} placeholder="otterroute@example.com" />
          </Field>
          <Field label={tr("nt.notify")}>
            <ThresholdSelect value={cfg.email.threshold} onChange={(threshold) => setEmail({ threshold })} />
          </Field>
        </div>
        <Field label={tr("nt.recipients")} hint={tr("nt.recipientsHint")}>
          <textarea rows={3} value={to} onChange={(e) => setTo(e.target.value)} placeholder="nome@example.com" />
        </Field>
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => test("email")} disabled={dirty || testing !== null} title={dirty ? tr("nt.saveFirst") : undefined}>
            {testing === "email" ? tr("nt.sending") : tr("nt.sendTest")}
          </button>
        </div>
      </div>

      <div className="card">
        <h2>{tr("nt.telegram")}</h2>
        <label className="check">
          <input type="checkbox" checked={cfg.telegram.enabled} onChange={(e) => setTg({ enabled: e.target.checked })} />
          {tr("nt.tgOn")}
        </label>
        <div className="row">
          <Field label={tr("nt.token")} hint={info.has_telegram_token ? tr("nt.tokenSaved") : tr("nt.tokenGet")}>
            <input type="password" value={token} onChange={(e) => setToken(e.target.value)} placeholder={info.has_telegram_token ? "••••••••" : "123456:ABC…"} autoComplete="new-password" />
          </Field>
          <Field label={tr("nt.notify")}>
            <ThresholdSelect value={cfg.telegram.threshold} onChange={(threshold) => setTg({ threshold })} />
          </Field>
        </div>
        <Field label={tr("nt.chat")} hint={tr("nt.chatHint")}>
          <textarea rows={3} value={chats} onChange={(e) => setChats(e.target.value)} placeholder="123456789" />
        </Field>
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => test("telegram")} disabled={dirty || testing !== null} title={dirty ? tr("nt.saveFirst") : undefined}>
            {testing === "telegram" ? tr("nt.sending") : tr("nt.sendTest")}
          </button>
        </div>
      </div>

      <div className="card">
        <h2>{tr("nt.rules")}</h2>
        <div className="row">
          <Field label={tr("nt.delay")} hint={tr("nt.delayHint")}>
            <input value={String(cfg.delay_min)} onChange={(e) => setCfg({ ...cfg, delay_min: num(e.target.value) })} inputMode="numeric" />
          </Field>
          <Field label={tr("nt.reminder")} hint={tr("nt.reminderHint")}>
            <input value={String(cfg.reminder_hours)} onChange={(e) => setCfg({ ...cfg, reminder_hours: num(e.target.value) })} inputMode="numeric" />
          </Field>
        </div>
        <label className="check">
          <input type="checkbox" checked={cfg.recovery} onChange={(e) => setCfg({ ...cfg, recovery: e.target.checked })} />
          {tr("nt.recovery")}
        </label>
        <div className="nav">
          <span />
          <button className="primary" onClick={save} disabled={busy || !dirty}>
            {busy ? tr("common.saving") : tr("common.save")}
          </button>
        </div>
      </div>

      <div className="card">
        <h2>{tr("nt.last")}</h2>
        <DataTable
          columns={columns}
          rows={log}
          rowKey={(l) => `${l.at}${l.channel}${l.kind}${l.title}`}
          empty={tr("nt.none")}
          initialSort={{ key: "at", dir: "desc" }}
          expand={(l) => (l.error ? <div className="box bad">{l.error}</div> : null)}
        />
      </div>
    </Page>
  );
}
