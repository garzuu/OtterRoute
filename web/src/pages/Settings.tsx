import { useState } from "react";
import { api, type PanelState } from "../api";
import { useAuth, needScope } from "../auth";
import { Field, Page } from "../ui";
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
    if (!Number.isInteger(n) || n < 1 || n > 65535) throw new Error("Porta non valida (1–65535).");
    return n;
  };

  const save = async (h: string, s: string) => {
    setBusy(true);
    setMsg(null);
    try {
      await api.saveSettings(parse(h), parse(s));
      await refresh();
      setMsg({ ok: true, text: "Salvato. Ricontrolla i domini per aggiornare lo stato." });
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
      setAcmeMsg({ ok: true, text: acme.enabled ? "Salvato. Il nodo sta ottenendo i certificati: lo stato compare in Domini." : "Salvato: i certificati automatici sono spenti." });
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
        text: host ? `Attivo: apri ${r.url}${r.warnings.length ? ` — Attenzione: ${r.warnings.join(" ")}` : ""}` : "Disattivato: il pannello resta solo sulla porta locale.",
      });
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
  const setWin = (w: { window_start?: number; window_end?: number }) => runUp(() => api.saveUpdates({ ...upd, ...w }), "Salvato.");
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
      setUpMsg({ ok: true, text: "Aggiornamento avviato: il nodo si riavvia da solo. Questa pagina si ricarica quando è tornato." });
      // il pannello si riavvia con il nodo: si aspetta che risponda con la nuova versione
      const started = Date.now();
      const poll = setInterval(async () => {
        try {
          const r = await fetch("/api/session");
          if (r.ok && Date.now() - started > 4000) {
            clearInterval(poll);
            window.location.reload();
          }
        } catch {
          /* il nodo si sta riavviando */
        }
      }, 2000);
    } catch (e) {
      setUpMsg({ ok: false, text: (e as Error).message });
      setUpBusy(false);
    }
  };
  const HOW: Record<string, string> = {
    docker: "docker pull ghcr.io/garzuu/otterroute:" + (up.latest?.version ?? "VERSIONE") + "\ndocker stop otterroute && docker rm otterroute\n# rilancia lo STESSO comando run, con lo stesso volume /data",
    service: "Scarica la release per la tua piattaforma dalla pagina delle release, sostituisci l’eseguibile e riavvia il servizio (systemctl restart otterroute).",
    binary: "Scarica la release per la tua piattaforma dalla pagina delle release, sostituisci l’eseguibile e riavvia il nodo.",
    source: "git pull && cargo build --release && (cd web && npm ci && npm run build)",
  };

  return (
    <Page title="Impostazioni" lead="Le porte standard del nodo. Cambiale solo se il tuo ambiente lo richiede.">
      <div className="card">
        <h2>Porte</h2>
        <div className="row">
          <Field label="HTTP" hint="Traffico pubblico dei domini. È la porta usata per verificare che i domini arrivino al nodo.">
            <input value={http} onChange={(e) => setHttp(e.target.value.replace(/\D/g, ""))} inputMode="numeric" disabled={!canWrite} />
          </Field>
          <Field label="HTTPS" hint="In arrivo: per ora non è ancora servito.">
            <input value={https} onChange={(e) => setHttps(e.target.value.replace(/\D/g, ""))} inputMode="numeric" disabled={!canWrite} />
          </Field>
        </div>
        <Field label="Pannello" hint="Solo su localhost, non va mai esposto. Si cambia all’avvio con OTR_ADMIN_LISTEN.">
          <input value="9090" disabled />
        </Field>
        {state.listen_port !== state.http_port && (
          <div className="box warn">
            Il nodo è in ascolto sulla porta <code>{state.listen_port}</code> (<code>OTR_LISTEN</code>), ma i domini vengono
            verificati sulla <code>{state.http_port}</code>. Va bene se un proxy o il port forwarding inoltra l’una all’altra;
            altrimenti allineale.
          </div>
        )}
        {!canWrite && <div className="box">Sola lettura: per modificare le impostazioni serve lo scope <code>settings:write</code>.</div>}
        {msg && <div className={`box ${msg.ok ? "good" : "bad"}`}>{msg.text}</div>}
        <div className="nav">
          {custom ? (
            <button className="ghost" onClick={restore} disabled={busy || !canWrite}>
              Ripristina 80 / 443
            </button>
          ) : (
            <span />
          )}
          <button className="primary" onClick={() => save(http, https)} disabled={busy || !dirty || !canWrite} title={canWrite ? undefined : needScope("settings:write")}>
            {busy ? "Salvo…" : "Salva"}
          </button>
        </div>
      </div>
      <div className="card">
        <h2>HTTPS automatico</h2>
        <p className="muted">
          Il nodo ottiene e rinnova da solo un certificato gratuito (Let’s Encrypt, sfida HTTP-01) per ogni dominio verificato. Serve che il dominio arrivi a questo nodo sulla porta 80 <strong>da Internet</strong>: dietro un proxy o una CDN il certificato si gestisce lì.
        </p>
        {!state.https_listening && (
          <div className="box warn">
            Il nodo non è in ascolto per HTTPS (porta 443 non disponibile o <code>OTR_HTTPS_LISTEN</code> vuoto): i certificati si ottengono comunque, ma non si servono.
          </div>
        )}
        <label className="check">
          <input type="checkbox" checked={acme.enabled} onChange={(e) => setAcme({ ...acme, enabled: e.target.checked })} disabled={!canWrite} />
          <span>Ottieni e rinnova i certificati in automatico. Attivandolo accetti i termini di servizio della CA (Let’s Encrypt).</span>
        </label>
        <div className="row">
          <Field label="Email di contatto" hint="Facoltativa: la CA la usa per avvisarti di scadenze e problemi.">
            <input value={acme.email} onChange={(e) => setAcme({ ...acme, email: e.target.value })} placeholder="nome@example.com" disabled={!canWrite} />
          </Field>
        </div>
        <label className="check">
          <input type="checkbox" checked={acme.staging} onChange={(e) => setAcme({ ...acme, staging: e.target.checked })} disabled={!canWrite} />
          <span>Usa l’ambiente di prova (staging): i certificati non sono validi nei browser, ma non hai limiti di richieste. Utile per provare.</span>
        </label>
        {acmeMsg && <div className={`box ${acmeMsg.ok ? "good" : "bad"}`}>{acmeMsg.text}</div>}
        <div className="nav">
          <span />
          <button className="primary" onClick={saveAcme} disabled={acmeBusy || !acmeDirty || !canWrite} title={canWrite ? undefined : needScope("settings:write")}>
            {acmeBusy ? "Salvo…" : "Salva"}
          </button>
        </div>
      </div>
      <div className="card">
        <h2>Pannello in HTTPS</h2>
        <p className="muted">
          Di norma il pannello risponde solo sulla porta locale (<code>127.0.0.1:9090</code>). Qui puoi servirlo anche in <strong>HTTPS</strong> su un dominio di questo nodo, con il suo certificato, per usarlo da browser senza tunnel. Il dominio deve avere un certificato in uso e non servire file.
        </p>
        <div className="box warn">
          Il pannello diventa raggiungibile da Internet: proteggilo con password lunghe e con la verifica in due passaggi obbligatoria (Utenti → Sicurezza). Con Cloudflare usa SSL <strong>Full</strong>, non Flexible.
        </div>
        {adminHost ? (
          <p>
            Attivo su <code>{adminHost}</code>. L’HTTP di quel dominio reindirizza a HTTPS.
          </p>
        ) : eligible.length === 0 ? (
          <p className="muted">Nessun dominio adatto: serve un dominio con un certificato in uso (Domini → HTTPS) e senza instradamenti.</p>
        ) : (
          <Field label="Dominio del pannello">
            <select value={pick} onChange={(e) => setPick(e.target.value)} disabled={!canWrite}>
              <option value="">Scegli un dominio…</option>
              {eligible.map((d) => (
                <option key={d.host} value={d.host}>
                  {d.host}
                </option>
              ))}
            </select>
          </Field>
        )}
        {adminMsg && <div className={`box ${adminMsg.ok ? "good" : "bad"}`}>{adminMsg.text}</div>}
        <div className="nav">
          <span />
          {adminHost ? (
            <button className="secondary" onClick={() => setAdmin(null)} disabled={adminBusy || !canWrite}>
              Disattiva
            </button>
          ) : (
            <button className="primary" onClick={() => setAdmin(pick)} disabled={adminBusy || !pick || !canWrite} title={canWrite ? undefined : needScope("settings:write")}>
              Attiva
            </button>
          )}
        </div>
      </div>
      <div className="card">
        <h2>Aggiornamenti</h2>
        <p>
          Versione in uso: <code>{up.current}</code> · installazione: <strong>{{ docker: "Docker", service: "servizio", binary: "binario", source: "sorgenti" }[up.kind]}</strong>
        </p>
        {up.available && up.latest ? (
          <div className="box warn">
            <strong>È disponibile la versione {up.latest.version}.</strong>{" "}
            <a href={up.latest.url} target="_blank" rel="noreferrer">
              Note della release ↗
            </a>
            {up.latest.notes && <pre className="small-text" style={{ whiteSpace: "pre-wrap", margin: "8px 0 0" }}>{up.latest.notes}</pre>}
            <div className="small-text" style={{ marginTop: 8 }}>
              <strong>Come aggiornare:</strong>
              <pre style={{ whiteSpace: "pre-wrap", margin: "4px 0 0" }}>{HOW[up.kind]}</pre>
              Prima fai un backup della cartella di stato. Le sessioni di accesso si perdono al riavvio.
            </div>
            {up.can_self_update ? (
              <div className="nav" style={{ marginTop: 10 }}>
                <span className="muted small-text">
                  Scarica il pacchetto, ne verifica checksum e firma, lo prova, salva un backup e riavvia il nodo; se non parte bene torna alla versione precedente.
                </span>
                <button className="primary" onClick={applyNow} disabled={upBusy || up.apply.running || !canWrite}>
                  {up.apply.running ? `In corso: ${up.apply.step}…` : "Aggiorna ora"}
                </button>
              </div>
            ) : (
              up.self_update_blocked && <div className="muted small-text" style={{ marginTop: 8 }}>Aggiornamento automatico non disponibile: {up.self_update_blocked}.</div>
            )}
            {up.apply.error && <div className="box bad" style={{ marginTop: 8 }}>{up.apply.error}</div>}
          </div>
        ) : (
          <div className="box good">{up.checked_at ? "Sei alla versione più recente." : "Ancora nessun controllo."}</div>
        )}
        {up.error && <div className="box bad">Ultimo controllo non riuscito: {up.error}</div>}
        {up.checked_at > 0 && <p className="muted small-text">Ultimo controllo: {new Date(up.checked_at * 1000).toLocaleString("it-IT")}</p>}
        {up.env_disabled && <div className="box">Il controllo è disattivato da <code>OTR_UPDATE_CHECK=off</code>: il nodo non contatta GitHub.</div>}
        <label className="check">
          <input type="checkbox" checked={upd.check} disabled={!canWrite || up.env_disabled || upBusy} onChange={(e) => runUp(() => api.saveUpdates({ ...upd, check: e.target.checked }), "Salvato.")} />
          <span>Cerca ogni giorno le nuove versioni (una richiesta alle release pubbliche di GitHub, senza inviare dati del nodo).</span>
        </label>
        <label className="check">
          <input type="checkbox" checked={upd.prerelease} disabled={!canWrite || up.env_disabled || upBusy} onChange={(e) => runUp(() => api.saveUpdates({ ...upd, prerelease: e.target.checked }), "Salvato.")} />
          <span>Proponi anche le versioni di prova (pre-release).</span>
        </label>
        {up.can_self_update && (
          <>
            <label className="check">
              <input type="checkbox" checked={upd.auto} disabled={!canWrite || upBusy} onChange={(e) => runUp(() => api.saveUpdates({ ...upd, auto: e.target.checked }), "Salvato.")} />
              <span>
                Applica da solo le versioni di <strong>correzione</strong> (es. 0.1.x) dalle <strong>{String(upd.window_start).padStart(2, "0")}:00</strong> alle <strong>{String(upd.window_end).padStart(2, "0")}:00</strong> (ora del nodo). Le versioni minori e maggiori restano manuali.
              </span>
            </label>
            {upd.auto && (
              <div className="row">
                <Field label="Dalle ore">
                  <input value={String(upd.window_start)} inputMode="numeric" onChange={(e) => setWin({ window_start: Math.min(23, Number(e.target.value.replace(/\D/g, "") || 0)) })} />
                </Field>
                <Field label="Alle ore">
                  <input value={String(upd.window_end)} inputMode="numeric" onChange={(e) => setWin({ window_end: Math.min(23, Number(e.target.value.replace(/\D/g, "") || 0)) })} />
                </Field>
              </div>
            )}
          </>
        )}
        {up.rollback && <div className="box warn">L’aggiornamento alla {up.rollback.to} è stato annullato: {up.rollback.reason}</div>}
        {upMsg && <div className={`box ${upMsg.ok ? "good" : "bad"}`}>{upMsg.text}</div>}
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => runUp(() => api.checkUpdate(), "Controllo eseguito.")} disabled={upBusy || !canWrite || up.env_disabled}>
            {upBusy ? "Controllo…" : "Controlla ora"}
          </button>
        </div>
      </div>
    </Page>
  );
}
