import { useState } from "react";
import { api, domainStatus, type CheckResult, type DomainInfo, type PanelState } from "../api";
import { useAuth } from "../auth";
import { DataTable, type Column } from "../DataTable";
import { DeleteButton, EmptyState, Field, Illus, Modal, Page, Stages, fmtTime } from "../ui";

const isLocal = (h: string) => h === "localhost" || h.endsWith(".localhost");

export const DOMAIN_BADGE = {
  pending: { cls: "badge warn", label: "In attesa" },
  verified: { cls: "badge good", label: "Verificato" },
  dns_error: { cls: "badge bad", label: "Errore DNS" },
  unreachable: { cls: "badge bad", label: "Nodo non raggiungibile" },
} as const;

/** Cosa deve succedere perché il dominio sia valido. */
function Hint(props: { host: string; port: number }) {
  if (isLocal(props.host)) return null;
  return (
    <div className="dnsbox">
      <div>
        Il dominio deve <strong>risolvere</strong> e le richieste sulla porta <code>{props.port}</code> devono{" "}
        <strong>arrivare a questo nodo</strong>, con l’instradamento che preferisci (proxy, load balancer, tunnel,
        port forwarding). Il nodo non deve avere un IP particolare.
      </div>
      <div className="muted">
        Per provare a mano da qualunque macchina (deve rispondere <code>{"{"}"otterroute":true…{"}"}</code>):
        <br />
        <code>
          curl -s http://{props.host}
          {props.port === 80 ? "" : `:${props.port}`}/.well-known/otterroute/check?nonce=prova
        </code>
      </div>
      <div className="muted">La propagazione DNS può richiedere fino a 48 ore.</div>
    </div>
  );
}

export function Domains(props: { state: PanelState; refresh: () => Promise<void> }) {
  const { state, refresh } = props;
  const { domains } = state.panel;
  const { can } = useAuth();
  const canWrite = can("domains:write");
  const [adding, setAdding] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const run = async (key: string, fn: () => Promise<unknown>) => {
    setBusy(key);
    setErr(null);
    try {
      await fn();
      await refresh();
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const columns: Column<DomainInfo>[] = [
    {
      key: "host",
      header: "Dominio",
      sort: (d) => d.host,
      render: (d) => <code className="big">{d.host}</code>,
    },
    {
      key: "status",
      header: "Stato",
      sort: (d) => DOMAIN_BADGE[domainStatus(d)].label,
      render: (d) => {
        const b = DOMAIN_BADGE[domainStatus(d)];
        return <span className={b.cls}>{b.label}</span>;
      },
    },
    {
      key: "records",
      header: "Risolve a",
      render: (d) => (d.records.length ? <code className="small-text">{d.records.slice(0, 2).join(", ")}</code> : <span className="muted">—</span>),
    },
    {
      key: "since",
      header: "Da",
      sort: (d) => d.since ?? "",
      render: (d) => <span className="muted">{d.since ? fmtTime(d.since) : "—"}</span>,
    },
  ];

  return (
    <Page
      title="Domini"
     
      lead="Censisci i domini che instradi verso questo nodo. Un dominio è utilizzabile solo dopo aver verificato che risolva e che arrivi davvero qui."
    >
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>Domini censiti</h2>
          {canWrite && domains.length > 0 && (
            <button className="primary small" onClick={() => setAdding(true)}>
              Nuovo dominio
            </button>
          )}
        </div>
        <DataTable
          columns={columns}
          rows={domains}
          rowKey={(d) => d.host}
          empty={
            <EmptyState
              image="search"
              title="Ancora nessun dominio"
              text="Censisci un dominio per verificare che risolva e che arrivi a questo nodo."
              action={canWrite ? { label: "Nuovo dominio", onClick: () => setAdding(true) } : undefined}
            />
          }
          searchText={(d) => `${d.host} ${DOMAIN_BADGE[domainStatus(d)].label}`}
          searchPlaceholder="Cerca dominio…"
          initialSort={{ key: "host", dir: "asc" }}
          defaultExpanded={(d) => domainStatus(d) !== "verified"}
          actions={!canWrite ? undefined : (d) => (
            <>
              <button className="secondary small" onClick={() => run(`check:${d.host}`, () => api.checkDomain(d.host))} disabled={busy === `check:${d.host}`}>
                {busy === `check:${d.host}` ? "Controllo…" : "Ricontrolla"}
              </button>
              <DeleteButton onConfirm={() => run(`del:${d.host}`, () => api.deleteDomain(d.host))} />
            </>
          )}
          expand={(d) => {
            const status = domainStatus(d);
            return (
              <div className="details">
                {(status === "dns_error" || status === "unreachable") && (
                  <div className="box bad">
                    {d.message}
                    <div className="small-text">
                      Il dominio era valido e ora non lo è più. Gli instradamenti continuano a essere serviti, ma
                      controlla il DNS e il tuo instradamento verso il nodo.
                    </div>
                  </div>
                )}
                {d.stages.length > 0 ? <Stages stages={d.stages} /> : <div className="muted">{d.message}</div>}
                {!d.verified && <Hint host={d.host} port={state.http_port} />}
              </div>
            );
          }}
        />
      </div>

      {adding && (
        <NewDomain
          state={state}
          onClose={() => setAdding(false)}
          onSaved={async () => {
            setAdding(false);
            await refresh();
          }}
        />
      )}
    </Page>
  );
}

export function DomainForm(props: { state: PanelState; onCancel?: () => void; cancelLabel?: string; onSaved: () => Promise<void> }) {
  const { state } = props;
  const [host, setHost] = useState("");
  const [result, setResult] = useState<{ host: string; result: CheckResult } | null>(null);
  const [busy, setBusy] = useState<"test" | "save" | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const test = async () => {
    setBusy("test");
    setErr(null);
    setResult(null);
    try {
      setResult(await api.testDomain(host));
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const save = async () => {
    setBusy("save");
    setErr(null);
    try {
      await api.addDomain(host);
      await props.onSaved();
    } catch (e) {
      setErr((e as Error).message);
      setBusy(null);
    }
  };

  const shown = result && result.host === host.trim().toLowerCase().replace(/\.$/, "") ? result.result : null;
  return (
    <>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void test();
        }}
      >
        <Field label="Dominio">
          <input
            value={host}
            onChange={(e) => {
              setHost(e.target.value);
              setResult(null);
            }}
            placeholder="media.azienda.it"
            spellCheck={false}
            autoFocus
          />
        </Field>
        {host.trim() && <Hint host={host.trim().toLowerCase()} port={state.http_port} />}
        <div className="mt">
          <button className="secondary" disabled={!host.trim() || busy !== null}>
            {busy === "test" ? "Verifico…" : "Verifica configurazione"}
          </button>
        </div>
      </form>

      {shown && (
        <>
          <Stages stages={shown.stages} />
          <div className={`box ${shown.ok ? "good verified" : "warn"}`}>
            {shown.ok ? (
              <>
                <Illus name="verified" width={44} />
                <span>Il dominio arriva a questo nodo.</span>
              </>
            ) : (
              "Non ancora raggiungibile. Puoi censirlo ora: il nodo lo ricontrolla da solo e diventa utilizzabile appena passa la verifica."
            )}
          </div>
        </>
      )}
      {err && <div className="box bad">{err}</div>}
      <div className="nav">
        {props.onCancel ? (
          <button className="ghost" onClick={props.onCancel}>
            {props.cancelLabel ?? "Annulla"}
          </button>
        ) : (
          <span />
        )}
        <button className="primary" onClick={save} disabled={!host.trim() || busy !== null}>
          {busy === "save" ? "Salvo…" : shown && !shown.ok ? "Censisci comunque" : "Censisci dominio"}
        </button>
      </div>
    </>
  );
}

function NewDomain(props: { state: PanelState; onClose: () => void; onSaved: () => Promise<void> }) {
  return (
    <Modal title="Nuovo dominio" onClose={props.onClose}>
      <DomainForm state={props.state} onCancel={props.onClose} onSaved={props.onSaved} />
    </Modal>
  );
}
