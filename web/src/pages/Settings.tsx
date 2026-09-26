import { useState } from "react";
import { api, type PanelState } from "../api";
import { loc } from "../i18n";
import { useAuth, needScope } from "../auth";
import { rich, tr } from "../i18n";
import { Field, Page } from "../ui";
import { BackupCard } from "./BackupCard";
import type { AcmeSettings } from "../api";

const DEFAULTS = { http: 80, https: 443 };

export function Settings(props: { state: PanelState; refresh: () => Promise<void> }) {
  const { state, refresh } = props;
  const { can } = useAuth();
  const canWrite = can("settings:write");
  const [http, setHttp] = useState(String(state.http_port));
  const [https, setHttps] = useState(String(state.https_port));
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  const dirty = http !== String(state.http_port) || https !== String(state.https_port);
  const custom = state.http_port !== DEFAULTS.http || state.https_port !== DEFAULTS.https;

  const parse = (v: string): number => {
    const n = Number(v);
    if (!Number.isInteger(n) || n < 1 || n > 65535) throw new Error(tr("st.badPort"));
    return n;
  };

  const save = async (h: string, s: string) => {
    setBusy(true);
    setMsg(null);
    try {
      await api.saveSettings(parse(h), parse(s));
      await refresh();
      setMsg({ ok: true, text: tr("st.saved") });
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };

  const restore = () => {
    setHttp(String(DEFAULTS.http));
    setHttps(String(DEFAULTS.https));
    void save(String(DEFAULTS.http), String(DEFAULTS.https));
  };

  const acme0 = state.panel.settings.acme;
  const [acme, setAcme] = useState<AcmeSettings>(acme0);
  const [acmeMsg, setAcmeMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [acmeBusy, setAcmeBusy] = useState(false);
  const acmeDirty = JSON.stringify(acme) !== JSON.stringify(acme0);
  const saveAcme = async () => {
    setAcmeBusy(true);
    setAcmeMsg(null);
    try {
      await api.saveHttps(acme);
      await refresh();
      setAcmeMsg({ ok: true, text: acme.enabled ? tr("st.acmeOn") : tr("st.acmeOff") });
    } catch (e) {
      setAcmeMsg({ ok: false, text: (e as Error).message });
    } finally {
      setAcmeBusy(false);
    }
  };

  // pannello in HTTPS: solo i domini con certificato in uso e senza instradamenti
  const adminHost = state.panel.settings.admin_host;
  const eligible = state.panel.domains.filter(
    (d) => state.certs.find((c) => c.host === d.host)?.serving && !state.panel.rules.some((r) => r.domain === d.host),
  );
  const [pick, setPick] = useState("");
  const [adminMsg, setAdminMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [adminBusy, setAdminBusy] = useState(false);
  const setAdmin = async (host: string | null) => {
    setAdminBusy(true);
    setAdminMsg(null);
    try {
      const r = await api.setAdminHost(host);
      await refresh();
      setAdminMsg({
        ok: true,
        text: host ? `${tr("st.adminOn", { url: r.url ?? "" })}${r.warnings.length ? tr("st.adminWarn", { w: r.warnings.join(" ") }) : ""}` : tr("st.adminOff"),
      });
    } catch (e) {
      setAdminMsg({ ok: false, text: (e as Error).message });
    } finally {
      setAdminBusy(false);
    }
  };

  const [allowText, setAllowText] = useState(state.panel.settings.admin_allow.join("\n"));
  const saveAllow = async () => {
    setAdminBusy(true);
    setAdminMsg(null);
    try {
      const r = await api.setAdminAllow(allowText.split(/[\s,]+/).filter(Boolean));
      await refresh();
      setAllowText(r.list.join("\n"));
      setAdminMsg({ ok: true, text: r.list.length ? tr("st.allowSaved") : tr("st.allowNone") });
    } catch (e) {
      setAdminMsg({ ok: false, text: (e as Error).message });
    } finally {
      setAdminBusy(false);
    }
  };

  const up = state.update;
  const [upBusy, setUpBusy] = useState(false);
  const [upMsg, setUpMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const upd = state.panel.settings.updates;
  const setWin = (w: { window_start?: number; window_end?: number }) => runUp(() => api.saveUpdates({ ...upd, ...w }), tr("st.savedShort"));
  const runUp = async (fn: () => Promise<unknown>, okText: string) => {
    setUpBusy(true);
    setUpMsg(null);
    try {
      await fn();
      await refresh();
      setUpMsg({ ok: true, text: okText });
    } catch (e) {
      setUpMsg({ ok: false, text: (e as Error).message });
    } finally {
      setUpBusy(false);
    }
  };
  const applyNow = async () => {
    setUpBusy(true);
    setUpMsg(null);
    try {
      await api.applyUpdate();
    } catch (e) {
      setUpMsg({ ok: false, text: (e as Error).message });
      setUpBusy(false);
      return;
    }
    setUpMsg({ ok: true, text: tr("st.upRunning") });
    // si segue lo stato reale: errore (resta com'è), riavvio (il nodo cade e torna) o completamento
    const started = Date.now();
    let wasDown = false;
    const poll = setInterval(async () => {
      try {
        const r = await fetch("/api/panel");
        if (!r.ok) throw new Error(tr("st.notAvailable"));
        const p = (await r.json()) as PanelState;
        if (wasDown || p.update.current !== up.current) {
          clearInterval(poll);
          window.location.reload();
        } else if (!p.update.apply.running && p.update.apply.error) {
          clearInterval(poll);
          setUpMsg({ ok: false, text: p.update.apply.error });
          setUpBusy(false);
          await refresh();
        } else if (p.update.apply.running) {
          setUpMsg({ ok: true, text: tr("st.upStep", { step: p.update.apply.step }) });
        } else if (Date.now() - started > 60000) {
          clearInterval(poll);
          setUpBusy(false);
        }
      } catch {
        wasDown = true; // il nodo si sta riavviando: alla prima risposta si ricarica
        setUpMsg({ ok: true, text: tr("st.upRestart") });
      }
    }, 1500);
  };
  const HOW: Record<string, string> = {
    docker: tr("st.howDocker", { v: up.latest?.version ?? tr("st.version") }),
    service: tr("st.howService"),
    binary: tr("st.howBinary"),
    source: "git pull && cargo build --release && (cd web && npm ci && npm run build)",
  };

  return (
    <Page title={tr("st.title")} lead={tr("st.lead")}>
      <div className="card">
        <h2>{tr("st.ports")}</h2>
        <div className="row">
          <Field label="HTTP" hint={tr("st.httpHint")}>
            <input value={http} onChange={(e) => setHttp(e.target.value.replace(/\D/g, ""))} inputMode="numeric" disabled={!canWrite} />
          </Field>
          <Field label="HTTPS" hint={tr("st.httpsHint")}>
            <input value={https} onChange={(e) => setHttps(e.target.value.replace(/\D/g, ""))} inputMode="numeric" disabled={!canWrite} />
          </Field>
        </div>
        <Field label={tr("st.panel")} hint={tr("st.panelHint")}>
          <input value="9090" disabled />
        </Field>
        {state.listen_port !== state.http_port && (
          <div className="box warn">{rich(tr("st.portMismatch", { listen: state.listen_port, http: state.http_port }))}</div>
        )}
        {!canWrite && <div className="box">{rich(tr("st.readOnly"))}</div>}
        {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
        <div className="nav">
          {custom ? (
            <button className="ghost" onClick={restore} disabled={busy || !canWrite}>
              {tr("st.restoreDefaults")}
            </button>
          ) : (
            <span />
          )}
          <button className="primary" onClick={() => save(http, https)} disabled={busy || !dirty || !canWrite} title={canWrite ? undefined : needScope("settings:write")}>
            {busy ? tr("common.saving") : tr("common.save")}
          </button>
        </div>
      </div>
      <div className="card">
        <h2>{tr("st.acmeTitle")}</h2>
        <p className="muted">
          {rich(tr("st.acmeLead"))}
        </p>
        {!state.https_listening && (
          <div className="box warn">
            {rich(tr("st.noHttpsListen"))}
          </div>
        )}
        <label className="check">
          <input type="checkbox" checked={acme.enabled} onChange={(e) => setAcme({ ...acme, enabled: e.target.checked })} disabled={!canWrite} />
          <span>{tr("st.acmeCheck")}</span>
        </label>
        <div className="row">
          <Field label={tr("st.acmeEmail")} hint={tr("st.acmeEmailHint")}>
            <input value={acme.email} onChange={(e) => setAcme({ ...acme, email: e.target.value })} placeholder="nome@example.com" disabled={!canWrite} />
          </Field>
        </div>
        <label className="check">
          <input type="checkbox" checked={acme.staging} onChange={(e) => setAcme({ ...acme, staging: e.target.checked })} disabled={!canWrite} />
          <span>{tr("st.staging")}</span>
        </label>
        {acmeMsg && <div className={`box ${acmeMsg.ok ? "good" : "bad"}`}>{acmeMsg.text}</div>}
        <div className="nav">
          <span />
          <button className="primary" onClick={saveAcme} disabled={acmeBusy || !acmeDirty || !canWrite} title={canWrite ? undefined : needScope("settings:write")}>
            {acmeBusy ? tr("common.saving") : tr("common.save")}
          </button>
        </div>
      </div>
      <div className="card">
        <h2>{tr("st.adminTitle")}</h2>
        <p className="muted">
          {rich(tr("st.adminLead"))}
        </p>
        <div className="box warn">
          {rich(tr("st.adminWarn2"))}
        </div>
        {adminHost ? (
          <p>
            {rich(tr("st.adminActive", { host: adminHost }))}
          </p>
        ) : eligible.length === 0 ? (
          <p className="muted">{tr("st.noEligible")}</p>
        ) : (
          <Field label={tr("st.adminDomain")}>
            <select value={pick} onChange={(e) => setPick(e.target.value)} disabled={!canWrite}>
              <option value="">{tr("st.pickDomain")}</option>
              {eligible.map((d) => (
                <option key={d.host} value={d.host}>
                  {d.host}
                </option>
              ))}
            </select>
          </Field>
        )}
        {adminHost && (
          <Field label={tr("st.allowLabel")}>
            <textarea rows={3} value={allowText} onChange={(e) => setAllowText(e.target.value)} disabled={!canWrite} placeholder={"203.0.113.7\n10.0.0.0/8\n2001:db8::/32"} />
            <span className="muted small-text">
              {tr("st.allowHint")}
            </span>
            <div className="nav">
              <span />
              <button className="secondary" onClick={saveAllow} disabled={adminBusy || !canWrite}>
                {tr("st.saveList")}
              </button>
            </div>
          </Field>
        )}
        {adminMsg && <div className={`box ${adminMsg.ok ? "good" : "bad"}`}>{adminMsg.text}</div>}
        <div className="nav">
          <span />
          {adminHost ? (
            <button className="secondary" onClick={() => setAdmin(null)} disabled={adminBusy || !canWrite}>
              {tr("common.disable")}
            </button>
          ) : (
            <button className="primary" onClick={() => setAdmin(pick)} disabled={adminBusy || !pick || !canWrite} title={canWrite ? undefined : needScope("settings:write")}>
              {tr("st.turnOn")}
            </button>
          )}
        </div>
      </div>
      <div className="card">
        <h2>{tr("st.upTitle")}</h2>
        <p>
          {rich(tr("st.inUse", { v: up.current, kind: tr(({ docker: "st.kDocker", service: "st.kService", binary: "st.kBinary", source: "st.kSource" } as const)[up.kind]) }))}
        </p>
        {up.available && up.latest ? (
          <div className="box warn">
            <strong>{tr("st.available", { v: up.latest.version })}</strong>{" "}
            <a href={up.latest.url} target="_blank" rel="noreferrer">
              {tr("st.notes")}
            </a>
            {up.latest.notes && <pre className="small-text" style={{ whiteSpace: "pre-wrap", margin: "8px 0 0" }}>{up.latest.notes}</pre>}
            <div className="small-text" style={{ marginTop: 8 }}>
              <strong>{up.can_self_update ? tr("st.orByHand") : tr("st.howTo")}</strong>
              <pre style={{ whiteSpace: "pre-wrap", margin: "4px 0 0" }}>{HOW[up.kind]}</pre>
              {tr("st.backupFirst")}
            </div>
            {up.can_self_update ? (
              <div className="nav" style={{ marginTop: 10 }}>
                <span className="muted small-text">
                  {tr("st.selfExplain")}
                </span>
                <button className="primary" onClick={applyNow} disabled={upBusy || up.apply.running || !canWrite}>
                  {up.apply.running ? tr("st.inProgress", { step: up.apply.step }) : tr("st.updateNow")}
                </button>
              </div>
            ) : (
              up.self_update_blocked && <div className="muted small-text" style={{ marginTop: 8 }}>{tr("st.blocked", { why: up.self_update_blocked })}</div>
            )}
            {up.apply.error && !upMsg && <div className="box bad" style={{ marginTop: 8 }}>{up.apply.error}</div>}
          </div>
        ) : (
          <div className="box good">{up.checked_at ? tr("st.latest") : tr("st.noCheck")}</div>
        )}
        {up.error && <div className="box bad">{tr("st.lastFail", { e: up.error })}</div>}
        {up.checked_at > 0 && <p className="muted small-text">{tr("st.lastCheck", { date: new Date(up.checked_at * 1000).toLocaleString(loc()) })}</p>}
        {up.env_disabled && <div className="box">{rich(tr("st.envOff"))}</div>}
        <label className="check">
          <input type="checkbox" checked={upd.check} disabled={!canWrite || up.env_disabled || upBusy} onChange={(e) => runUp(() => api.saveUpdates({ ...upd, check: e.target.checked }), tr("st.savedShort"))} />
          <span>{tr("st.dailyCheck")}</span>
        </label>
        <label className="check">
          <input type="checkbox" checked={upd.prerelease} disabled={!canWrite || up.env_disabled || upBusy} onChange={(e) => runUp(() => api.saveUpdates({ ...upd, prerelease: e.target.checked }), tr("st.savedShort"))} />
          <span>{tr("st.pre")}</span>
        </label>
        {up.can_self_update && (
          <>
            <label className="check">
              <input type="checkbox" checked={upd.auto} disabled={!canWrite || upBusy} onChange={(e) => runUp(() => api.saveUpdates({ ...upd, auto: e.target.checked }), tr("st.savedShort"))} />
              <span>
                {rich(tr("st.auto", { from: String(upd.window_start).padStart(2, "0"), to: String(upd.window_end).padStart(2, "0") }))}
              </span>
            </label>
            {upd.auto && (
              <div className="row">
                <Field label={tr("st.fromHour")}>
                  <input value={String(upd.window_start)} inputMode="numeric" onChange={(e) => setWin({ window_start: Math.min(23, Number(e.target.value.replace(/\D/g, "") || 0)) })} />
                </Field>
                <Field label={tr("st.toHour")}>
                  <input value={String(upd.window_end)} inputMode="numeric" onChange={(e) => setWin({ window_end: Math.min(23, Number(e.target.value.replace(/\D/g, "") || 0)) })} />
                </Field>
              </div>
            )}
          </>
        )}
        {up.rollback && <div className="box warn">{tr("st.rolledBack", { to: up.rollback.to, reason: up.rollback.reason })}</div>}
        {upMsg && <div className={`box ${upMsg.ok ? "good" : "bad"}`}>{upMsg.text}</div>}
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => runUp(() => api.checkUpdate(), tr("st.checked"))} disabled={upBusy || !canWrite || up.env_disabled}>
            {upBusy ? tr("bk2.checking") : tr("st.checkNow")}
          </button>
        </div>
      </div>
      <BackupCard />
    </Page>
  );
}
