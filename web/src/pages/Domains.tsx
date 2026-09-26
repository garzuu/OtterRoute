import { useState } from "react";
import { api, domainStatus, type CertInfo, type CheckResult, type DomainInfo, type PanelState } from "../api";
import { loc, rich, tr } from "../i18n";
import { useAuth } from "../auth";
import { DataTable, type Column } from "../DataTable";
import { DeleteButton, EmptyState, Field, Illus, Modal, Page, Stages, fmtTime } from "../ui";

// le etichette sono getter: si leggono al momento dell'uso e seguono la lingua
const CERT_BADGE: Record<CertInfo["status"], { cls: string; readonly label: string }> = {
  valid: { cls: "badge good", get label() { return tr("dm.cValid"); } },
  expiring: { cls: "badge warn", get label() { return tr("dm.cExpiring"); } },
  expired: { cls: "badge bad", get label() { return tr("dm.cExpired"); } },
  missing: { cls: "badge", get label() { return tr("dm.cMissing"); } },
  error: { cls: "badge bad", get label() { return tr("bk2.error"); } },
  issuing: { cls: "badge warn", get label() { return tr("dm.cIssuing"); } },
};

const fmtDay = (unix: number) => new Date(unix * 1000).toLocaleDateString(loc());

/** Certificato HTTPS di un dominio, redirect e caricamento manuale. */
function HttpsPanel(props: { d: DomainInfo; cert: CertInfo | undefined; auto: boolean; canWrite: boolean; run: (key: string, fn: () => Promise<unknown>) => Promise<void>; busy: string | null }) {
  const { d, cert, auto, canWrite, run, busy } = props;
  const [chain, setChain] = useState("");
  const [key, setKey] = useState("");
  const [open, setOpen] = useState(false);
  const serving = cert?.serving ?? false;
  return (
    <div className="details">
      <strong>HTTPS</strong>
      {cert ? (
        <div>
          <span className={CERT_BADGE[cert.status].cls}>{CERT_BADGE[cert.status].label}</span>{" "}
          {cert.not_after && <span className="muted">{tr("dm.expires", { date: fmtDay(cert.not_after) })}</span>}
          {cert.error && <div className="box bad">{cert.error.message}</div>}
        </div>
      ) : (
        <div className="muted">{tr("dm.noPublicCert")}</div>
      )}
      {!auto && <div className="muted">{tr("dm.autoOff")}</div>}
      {canWrite && (
        <div className="inline tight">
          {auto && cert && d.verified && (
            <button className="secondary small" disabled={busy === `cert:${d.host}` || cert.status === "issuing"} onClick={() => run(`cert:${d.host}`, () => api.issueCert(d.host))}>
              {cert.status === "issuing" ? tr("dm.issuingNow") : serving ? tr("dm.renew") : tr("dm.request")}
            </button>
          )}
          <button className="ghost small" onClick={() => setOpen(!open)}>
            {open ? tr("common.close") : tr("dm.upload")}
          </button>
        </div>
      )}
      {canWrite && (
        <label className="check">
          <input type="checkbox" checked={d.redirect_https} disabled={(!serving && !d.redirect_https) || busy === `redir:${d.host}`} onChange={(e) => run(`redir:${d.host}`, () => api.setRedirect(d.host, e.target.checked))} />
          <span>
            {tr("dm.redirect", { note: serving ? "" : tr("dm.redirectNote") })}
          </span>
        </label>
      )}
      {open && canWrite && (
        <div>
          <Field label={tr("dm.chain")}>
            <textarea rows={4} value={chain} onChange={(e) => setChain(e.target.value)} placeholder="-----BEGIN CERTIFICATE-----" spellCheck={false} />
          </Field>
          <Field label={tr("dm.key")} hint={tr("dm.keyHint")}>
            <textarea rows={4} value={key} onChange={(e) => setKey(e.target.value)} placeholder="-----BEGIN PRIVATE KEY-----" spellCheck={false} />
          </Field>
          <button
            className="secondary small"
            disabled={!chain.trim() || !key.trim() || busy === `up:${d.host}`}
            onClick={() =>
              run(`up:${d.host}`, async () => {
                await api.uploadCert(d.host, chain, key);
                setChain("");
                setKey("");
                setOpen(false);
              })
            }
          >
            {tr("dm.uploadBtn")}
          </button>
        </div>
      )}
    </div>
  );
}

const isLocal = (h: string) => h === "localhost" || h.endsWith(".localhost");

export const DOMAIN_BADGE = {
  pending: { cls: "badge warn", get label() { return tr("dm.pending"); } },
  verified: { cls: "badge good", get label() { return tr("dm.verified"); } },
  dns_error: { cls: "badge bad", get label() { return tr("dm.dnsError"); } },
  unreachable: { cls: "badge bad", get label() { return tr("dm.unreachable"); } },
} as const;

/** Cosa deve succedere perché il dominio sia valido. */
function Hint(props: { host: string; port: number }) {
  if (isLocal(props.host)) return null;
  return (
    <div className="dnsbox">
      <div>{rich(tr("dm.hint1", { port: props.port }))}</div>
      <div className="muted">
        {tr("dm.hint2", { json: '{"otterroute":true…}' })}
        <br />
        <code>
          curl -s http://{props.host}
          {props.port === 80 ? "" : `:${props.port}`}/.well-known/otterroute/check?nonce=prova
        </code>
      </div>
      <div className="muted">{tr("dm.hint3")}</div>
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

  const certOf = (host: string) => state.certs.find((c) => c.host === host);
  const auto = state.panel.settings.acme.enabled;
  const columns: Column<DomainInfo>[] = [
    {
      key: "host",
      header: tr("dm.colDomain"),
      sort: (d) => d.host,
      render: (d) => <code className="big">{d.host}</code>,
    },
    {
      key: "status",
      header: tr("bk2.colStatus"),
      sort: (d) => DOMAIN_BADGE[domainStatus(d)].label,
      render: (d) => {
        const b = DOMAIN_BADGE[domainStatus(d)];
        return <span className={b.cls}>{b.label}</span>;
      },
    },
    {
      key: "https",
      header: "HTTPS",
      sort: (d) => certOf(d.host)?.status ?? "",
      render: (d) => {
        const c = certOf(d.host);
        if (!c) return <span className="muted">—</span>;
        const b = CERT_BADGE[c.status];
        return (
          <span className={b.cls} title={c.not_after ? tr("dm.expiresCap", { date: fmtDay(c.not_after) }) : undefined}>
            {b.label}
          </span>
        );
      },
    },
    {
      key: "records",
      header: tr("dm.colResolves"),
      render: (d) => (d.records.length ? <code className="small-text">{d.records.slice(0, 2).join(", ")}</code> : <span className="muted">—</span>),
    },
    {
      key: "since",
      header: tr("dm.colSince"),
      sort: (d) => d.since ?? "",
      render: (d) => <span className="muted">{d.since ? fmtTime(d.since) : "—"}</span>,
    },
  ];

  return (
    <Page
      title={tr("nav.domains")}
      lead={tr("dm.lead")}
    >
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>{tr("dm.registered")}</h2>
          {canWrite && domains.length > 0 && (
            <button className="primary small" onClick={() => setAdding(true)}>
              {tr("dm.new")}
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
              title={tr("dm.none")}
              text={tr("dm.noneText")}
              action={canWrite ? { label: tr("dm.new"), onClick: () => setAdding(true) } : undefined}
            />
          }
          searchText={(d) => `${d.host} ${DOMAIN_BADGE[domainStatus(d)].label}`}
          searchPlaceholder={tr("dm.search")}
          initialSort={{ key: "host", dir: "asc" }}
          defaultExpanded={(d) => domainStatus(d) !== "verified"}
          actions={!canWrite ? undefined : (d) => (
            <>
              <button className="secondary small" onClick={() => run(`check:${d.host}`, () => api.checkDomain(d.host))} disabled={busy === `check:${d.host}`}>
                {busy === `check:${d.host}` ? tr("bk2.checking") : tr("bk2.recheck")}
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
                    <div className="small-text">{tr("dm.wasValid")}</div>
                  </div>
                )}
                {d.stages.length > 0 ? <Stages stages={d.stages} /> : <div className="muted">{d.message}</div>}
                {!d.verified && <Hint host={d.host} port={state.http_port} />}
                <HttpsPanel d={d} cert={certOf(d.host)} auto={auto} canWrite={canWrite} run={run} busy={busy} />
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
        <Field label={tr("dm.colDomain")}>
          <input
            value={host}
            onChange={(e) => {
              setHost(e.target.value);
              setResult(null);
            }}
            placeholder={tr("dm.placeholder")}
            spellCheck={false}
            autoFocus
          />
        </Field>
        {host.trim() && <Hint host={host.trim().toLowerCase()} port={state.http_port} />}
        <div className="mt">
          <button className="secondary" disabled={!host.trim() || busy !== null}>
            {busy === "test" ? tr("bk2.verifying") : tr("dm.verify")}
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
                <span>{tr("dm.reaches")}</span>
              </>
            ) : (
              tr("dm.notYet")
            )}
          </div>
        </>
      )}
      {err && <div className="box bad">{err}</div>}
      <div className="nav">
        {props.onCancel ? (
          <button className="ghost" onClick={props.onCancel}>
            {props.cancelLabel ?? tr("common.cancel")}
          </button>
        ) : (
          <span />
        )}
        <button className="primary" onClick={save} disabled={!host.trim() || busy !== null}>
          {busy === "save" ? tr("common.saving") : shown && !shown.ok ? tr("dm.saveAnyway") : tr("dm.register")}
        </button>
      </div>
    </>
  );
}

function NewDomain(props: { state: PanelState; onClose: () => void; onSaved: () => Promise<void> }) {
  return (
    <Modal title={tr("dm.new")} onClose={props.onClose}>
      <DomainForm state={props.state} onCancel={props.onClose} onSaved={props.onSaved} />
    </Modal>
  );
}
