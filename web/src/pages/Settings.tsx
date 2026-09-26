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
    </Page>
  );
}
