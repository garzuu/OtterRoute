import { useState } from "react";
import { api, type PanelState } from "../api";
import { useAuth, needScope } from "../auth";
import { Field, Page } from "../ui";

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
    </Page>
  );
}
