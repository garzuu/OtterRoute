import { useEffect, useRef, useState, type ReactNode } from "react";
import type { Alert } from "./alerts";
import { ROLE_LABEL } from "./auth";
import { docsHref } from "./ui";
import type { PageId } from "./Dashboard";
import { IconBell, IconCheck, IconChevronDown, IconLogout, IconMenu, IconUser } from "./icons";

/** Chiude il pannello con Esc o cliccando fuori. */
function usePopover() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && setOpen(false);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);
  return { open, setOpen, ref };
}

function Popover(props: { children: ReactNode; label: string }) {
  return (
    <div className="pop" role="dialog" aria-label={props.label}>
      {props.children}
    </div>
  );
}

/** Sezioni della guida utili per ogni pagina del pannello (le ancore sono quelle delle pagine). */
const HELP: Record<PageId, [string, string][]> = {
  overview: [
    ["Cosa si misura", "guide/statistics#cosa-si-misura"],
    ["La dashboard", "guide/statistics#la-dashboard"],
    ["Prometheus", "guide/statistics#prometheus"],
    ["Cosa dice X-Cache", "guide/cache#cosa-dice-x-cache"],
  ],
  domains: [
    ["Cosa controlla la verifica", "guide/domains-dns#cosa-controlla-la-verifica"],
    ["Quali record creare", "guide/domains-dns#quali-record-creare"],
    ["Propagazione e tempi", "guide/domains-dns#propagazione-e-tempi"],
    ["Gli stati di un dominio", "guide/domains-dns#gli-stati-di-un-dominio"],
    ["Provider DNS", "providers/#dns"],
    ["Problemi coi domini", "guide/troubleshooting#domini"],
  ],
  buckets: [
    ["I campi", "guide/buckets-s3#i-campi"],
    ["Path o virtual host", "guide/buckets-s3#path-oppure-virtual-host"],
    ["Permessi minimi", "guide/buckets-s3#permessi-minimi-sola-lettura"],
    ["Provider S3", "providers/#storage-s3"],
    ["Problemi coi bucket", "guide/troubleshooting#bucket"],
  ],
  routes: [
    ["Come si legge", "guide/routes#come-si-legge"],
    ["Regole di scelta", "guide/routes#regole-di-scelta"],
    ["La politica di cache", "guide/cache#la-politica-di-cache"],
    ["Problemi con file e 404", "guide/troubleshooting#instradamenti-e-file"],
  ],
  notifications: [
    ["Cosa viene notificato", "guide/notifications#cosa-viene-notificato"],
    ["Configurare Telegram", "guide/notifications#telegram"],
    ["Configurare l'email (SMTP)", "guide/notifications#email-smtp"],
    ["Soglie e ripristino", "guide/notifications#soglie-attesa-e-ripristino"],
  ],
  settings: [
    ["Porte standard", "guide/install#porte-standard"],
    ["Variabili d'ambiente", "reference/environment"],
    ["HTTPS e proxy", "guide/https-proxy"],
  ],
  users: [
    ["Ruoli e scope", "guide/users-2fa#ruoli-e-scope"],
    ["Aggiungere un utente", "guide/users-2fa#aggiungere-un-utente"],
    ["Rendere obbligatoria la 2FA", "guide/users-2fa#renderla-obbligatoria"],
    ["Registro delle attività", "guide/users-2fa#registro-delle-attivita"],
    ["Sicurezza", "guide/security"],
  ],
  profile: [
    ["Attivare la 2FA", "guide/users-2fa#verifica-in-due-passaggi-2fa"],
    ["Codici di recupero", "guide/users-2fa#accedere-con-la-2fa"],
    ["Perdere l'accesso", "guide/users-2fa#perdere-l-accesso"],
  ],
};

export function Topbar(props: {
  page: PageId;
  username: string;
  alerts: Alert[];
  onToggleSidebar: () => void;
  onAlert: (a: Alert) => void;
  role: string;
  onProfile: () => void;
  onLogout: () => void;
}) {
  const help = usePopover();
  const bell = usePopover();
  const user = usePopover();
  const errors = props.alerts.filter((a) => a.level === "error").length;
  const warns = props.alerts.filter((a) => a.level === "warn").length;
  const count = props.alerts.length;

  return (
    <header className="topbar">
      <button className="icon-btn ghost" onClick={props.onToggleSidebar} aria-label="Mostra o nascondi il menu">
        <IconMenu />
      </button>
      <div className="spacer" />

      {docsHref("") && (
        <div className="pop-wrap" ref={help.ref}>
          <button
            className="icon-btn ghost help-btn"
            onClick={() => {
              help.setOpen(!help.open);
              bell.setOpen(false);
              user.setOpen(false);
            }}
            aria-haspopup="dialog"
            aria-expanded={help.open}
            aria-label="Guida"
            title="Guida"
          >
            ?
          </button>
          {help.open && (
            <Popover label="Guida">
              <div className="pop-head">Guida</div>
              <div className="help-sub">In questa pagina</div>
              <div className="help-links">
                {HELP[props.page].map(([label, to]) => (
                  <a key={to} href={docsHref(to) ?? "#"} target="_blank" rel="noreferrer" onClick={() => help.setOpen(false)}>
                    {label} <span>↗</span>
                  </a>
                ))}
              </div>
              <div className="help-sub">Altro</div>
              <div className="help-links">
                <a href={docsHref("guide/troubleshooting") ?? "#"} target="_blank" rel="noreferrer">
                  Risoluzione dei problemi <span>↗</span>
                </a>
                <a href={docsHref("") ?? "#"} target="_blank" rel="noreferrer">
                  Tutta la guida <span>↗</span>
                </a>
              </div>
            </Popover>
          )}
        </div>
      )}

      <div className="pop-wrap" ref={bell.ref}>
        <button
          className="icon-btn ghost"
          onClick={() => {
            bell.setOpen(!bell.open);
            user.setOpen(false);
            help.setOpen(false);
          }}
          aria-haspopup="dialog"
          aria-expanded={bell.open}
          aria-label={count ? `Avvisi: ${count}` : "Nessun avviso"}
        >
          <IconBell />
          {count > 0 && <span className={errors ? "bell-count bad" : warns ? "bell-count" : "bell-count info"}>{count}</span>}
        </button>
        {bell.open && (
          <Popover label="Avvisi">
            <div className="pop-head">Avvisi</div>
            {count === 0 ? (
              <div className="pop-empty">
                <IconCheck /> Nessun avviso: è tutto in ordine.
              </div>
            ) : (
              <ul className="alert-list">
                {props.alerts.map((a) => (
                  <li key={a.id}>
                    <button
                      className="alert-item"
                      onClick={() => {
                        bell.setOpen(false);
                        props.onAlert(a);
                      }}
                    >
                      <span className={`alert-dot ${a.level}`} aria-hidden />
                      <span className="grow">
                        <strong>{a.title}</strong>
                        <span className="muted block small-text">{a.text}</span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </Popover>
        )}
      </div>

      <div className="pop-wrap" ref={user.ref}>
        <button
          className="userbtn ghost"
          onClick={() => {
            user.setOpen(!user.open);
            bell.setOpen(false);
            help.setOpen(false);
          }}
          aria-haspopup="menu"
          aria-expanded={user.open}
        >
          <img className="avatar" src="/brand/avatar.png" alt="" width={28} height={28} />
          <span className="uname">{props.username}</span>
          <IconChevronDown className="chev" />
        </button>
        {user.open && (
          <Popover label="Account">
            <div className="profile">
              <img src="/brand/avatar.png" alt="" width={48} height={48} />
              <div>
                <span className="muted small-text block">Connesso come</span>
                <strong>{props.username}</strong>
                <span className="muted small-text block">{ROLE_LABEL[props.role] ?? props.role}</span>
              </div>
            </div>
            <button
              className="pop-item"
              onClick={() => {
                user.setOpen(false);
                props.onProfile();
              }}
            >
              <IconUser /> Il mio profilo
            </button>
            <button className="pop-item" onClick={props.onLogout}>
              <IconLogout /> Esci
            </button>
          </Popover>
        )}
      </div>
    </header>
  );
}
