import { useEffect, useState, type ReactNode } from "react";
import { curLang, loc, tr } from "./i18n";
import { IconCheck, IconMinus, IconX } from "./icons";

export function Field(props: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="field">
      <span className="flabel">{props.label}</span>
      {props.children}
      {props.hint && <span className="hint">{props.hint}</span>}
    </label>
  );
}

/** Eliminazione in due passi, senza dialoghi del browser. */
export function DeleteButton(props: { onConfirm: () => void | Promise<void>; label?: string }) {
  const [armed, setArmed] = useState(false);
  const [busy, setBusy] = useState(false);
  if (!armed)
    return (
      <button className="ghost small" onClick={() => setArmed(true)}>
        {props.label ?? tr("common.delete")}
      </button>
    );
  return (
    <span className="actions">
      <button
        className="danger small"
        disabled={busy}
        onClick={async () => {
          setBusy(true);
          try {
            await props.onConfirm();
          } finally {
            setBusy(false);
            setArmed(false);
          }
        }}
      >
        {tr("common.confirm")}
      </button>
      <button className="ghost small" onClick={() => setArmed(false)}>
        {tr("common.cancel")}
      </button>
    </span>
  );
}

let docsBase: string | null = null;

/** Impostato all'arrivo della sessione: dove si trova la guida (vuoto = nessun link). */
export function setDocsBase(url: string | null | undefined) {
  docsBase = url ? (url.endsWith("/") ? url : `${url}/`) : null;
}

/** Indirizzo di una pagina (con ancora) della guida; null se non c'è una guida. */
export const docsHref = (page: string): string | null => (docsBase ? `${docsBase}${curLang() === "en" ? "en/" : ""}${page}` : null);

export function Page(props: { title: string; lead?: string; children: ReactNode }) {
  return (
    <section>
      <h1 className="ptitle">{props.title}</h1>
      {props.lead && <p className="lead">{props.lead}</p>}
      {props.children}
    </section>
  );
}

export const fmtBytes = (n: number): string => {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 ? 0 : v < 10 ? 2 : v < 100 ? 1 : 0;
  return `${v.toLocaleString(loc(), { maximumFractionDigits: digits, minimumFractionDigits: 0 })} ${units[i]}`;
};

export const fmtTime = (iso: string | null) =>
  iso ? new Date(iso).toLocaleString(loc(), { dateStyle: "short", timeStyle: "short" }) : tr("common.never");

/** Finestra sopra la pagina; si chiude con Esc o cliccando fuori. */
export function Modal(props: { title: string; onClose: () => void; wide?: boolean; children: ReactNode }) {
  const { onClose } = props;
  useEffect(() => {
    const on = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, [onClose]);
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={props.wide ? "modal wide" : "modal"} role="dialog" aria-modal="true" aria-label={props.title}>
        <div className="mhead">
          <h2>{props.title}</h2>
          <button className="ghost small" onClick={onClose} aria-label={tr("common.close")}>
            <IconX />
          </button>
        </div>
        {props.children}
      </div>
    </div>
  );
}

const ICON = { ok: <IconCheck />, fail: <IconX />, skip: <IconMinus /> } as const;

/** Elenco dei passaggi di un controllo, con esito e messaggio. */
export function Stages(props: { stages: { id: string; label: string; status: "ok" | "fail" | "skip"; message: string }[] }) {
  return (
    <ul className="stages">
      {props.stages.map((s) => (
        <li key={s.id} className={`stage ${s.status}`}>
          <span className="sicon" aria-hidden>
            {ICON[s.status]}
          </span>
          <span className="grow">
            <strong>{s.label}</strong>
            <span className="muted block">{s.message}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}

export type Illustration = "welcome" | "bucket" | "laptop" | "verified" | "search" | "sleeping";

/** Illustrazione della lontra. Nel tema scuro sta su una tessera chiara. */
export function Illus(props: { name: Illustration; width?: number }) {
  const w = props.width ?? 120;
  return (
    <span className="illus">
      <img src={`/brand/${props.name}.png`} alt="" width={w} loading="lazy" />
    </span>
  );
}

/** Stato vuoto: illustrazione, titolo, testo e (facoltativo) pulsante. */
export function EmptyState(props: {
  image: Illustration;
  title: string;
  text?: string;
  action?: { label: string; onClick: () => void };
}) {
  return (
    <div className="empty">
      <Illus name={props.image} />
      <h3>{props.title}</h3>
      {props.text && <p>{props.text}</p>}
      {props.action && (
        <button className="primary" onClick={props.action.onClick}>
          {props.action.label}
        </button>
      )}
    </div>
  );
}
