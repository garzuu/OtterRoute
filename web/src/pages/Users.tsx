import { useCallback, useEffect, useState } from "react";
import { api, type AuditEntry, type ScopeInfo, type TwoFactorPolicy, type UserInfo } from "../api";
import { ROLE_HELP, ROLE_LABEL, useAuth } from "../auth";
import { DataTable, type Column } from "../DataTable";
import { DeleteButton, EmptyState, Field, Modal, Page, fmtTime } from "../ui";

const ROLES = ["admin", "operator", "viewer", "custom"];

const ACTION_LABEL: Record<string, string> = {
  login: "Accesso",
  "login-2fa": "Verifica 2FA",
  setup: "Primo avvio",
  denied: "Permesso negato",
  "user.create": "Utente creato",
  "user.update": "Utente modificato",
  "user.delete": "Utente eliminato",
  "user.reset-password": "Password reimpostata",
  "user.reset-2fa": "2FA reimpostata",
  "policy.update": "Criterio di sicurezza",
  "password.change": "Password cambiata",
  "2fa.enable": "2FA attivata",
  "2fa.disable": "2FA disattivata",
  "2fa.recovery-codes": "Codici di recupero rigenerati",
  "2fa.confirm-failed": "Conferma 2FA fallita",
  "domain.add": "Dominio aggiunto",
  "domain.delete": "Dominio eliminato",
  "bucket.add": "Bucket aggiunto",
  "bucket.delete": "Bucket eliminato",
  "rule.add": "Instradamento creato",
  "rule.delete": "Instradamento eliminato",
  "rule.signed": "Link firmati (instradamento)",
  "link.create": "Link firmato creato",
  "link.rotate": "Chiave dei link ruotata",
  "cache.purge": "Cache svuotata",
  "cache.warm": "Cache precaricata",
  "notifications.update": "Notifiche modificate",
  "notifications.test": "Prova di notifica",
  "settings.update": "Impostazioni modificate",
};

const POLICY_LABEL: Record<TwoFactorPolicy, string> = {
  off: "Nessun obbligo (ognuno decide)",
  all: "Obbligatoria per tutti gli utenti",
  managers: "Obbligatoria per chi gestisce gli utenti",
};

type Tab = "users" | "audit";

export function Users() {
  const [tab, setTab] = useState<Tab>("users");
  return (
    <Page title="Utenti" lead="Chi può accedere al pannello e cosa può fare. Ogni azione è attribuita a chi l’ha compiuta.">
      <div className="toolbar">
        <div className="seg" role="tablist" aria-label="Sezione">
          <button role="tab" aria-selected={tab === "users"} className={tab === "users" ? "on" : ""} onClick={() => setTab("users")}>
            Utenti
          </button>
          <button role="tab" aria-selected={tab === "audit"} className={tab === "audit" ? "on" : ""} onClick={() => setTab("audit")}>
            Attività
          </button>
        </div>
      </div>
      {tab === "users" ? <UsersTab /> : <AuditTab />}
    </Page>
  );
}

function scopeSummary(u: UserInfo) {
  return u.role === "custom" ? `${u.scopes.length} ${u.scopes.length === 1 ? "permesso" : "permessi"}` : (ROLE_LABEL[u.role] ?? u.role);
}

function UsersTab() {
  const { user: me } = useAuth();
  const [users, setUsers] = useState<UserInfo[]>([]);
  const [scopes, setScopes] = useState<ScopeInfo[]>([]);
  const [policy, setPolicy] = useState<TwoFactorPolicy>("off");
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<UserInfo | null>(null);
  const [secret, setSecret] = useState<{ title: string; user: string; password: string } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [policyMsg, setPolicyMsg] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .users()
      .then((r) => {
        setUsers(r.users);
        setScopes(r.scopes);
        setPolicy(r.policy.require_2fa);
      })
      .catch((e: Error) => setErr(e.message));
  }, []);
  useEffect(load, [load]);

  const act = async (fn: () => Promise<unknown>) => {
    setErr(null);
    try {
      await fn();
      load();
    } catch (e) {
      setErr((e as Error).message);
    }
  };

  const columns: Column<UserInfo>[] = [
    {
      key: "user",
      header: "Utente",
      sort: (u) => u.username.toLowerCase(),
      render: (u) => (
        <span>
          <strong>{u.username}</strong> {u.username === me.username && <span className="badge">tu</span>}
        </span>
      ),
    },
    { key: "role", header: "Ruolo", sort: (u) => u.role, render: (u) => scopeSummary(u) },
    {
      key: "tfa",
      header: "2FA",
      sort: (u) => (u.two_factor ? 1 : 0),
      render: (u) => (u.two_factor ? <span className="badge good">Attiva</span> : <span className="muted">—</span>),
    },
    {
      key: "state",
      header: "Stato",
      sort: (u) => (u.disabled ? 2 : u.must_change_password ? 1 : 0),
      render: (u) =>
        u.disabled ? (
          <span className="badge bad">Disabilitato</span>
        ) : u.must_change_password ? (
          <span className="badge warn">Password temporanea</span>
        ) : (
          <span className="badge good">Attivo</span>
        ),
    },
    { key: "last", header: "Ultimo accesso", sort: (u) => u.last_login_at ?? "", render: (u) => <span className="muted">{fmtTime(u.last_login_at)}</span> },
  ];

  return (
    <>
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>Utenti</h2>
          <button className="primary small" onClick={() => setCreating(true)}>
            Nuovo utente
          </button>
        </div>
        <DataTable
          columns={columns}
          rows={users}
          rowKey={(u) => u.id}
          empty={<EmptyState image="welcome" title="Nessun utente" />}
          searchText={(u) => `${u.username} ${u.role}`}
          searchPlaceholder="Cerca utente…"
          initialSort={{ key: "user", dir: "asc" }}
          actions={(u) => {
            const self = u.username === me.username;
            return (
              <>
                <button className="secondary small" onClick={() => setEditing(u)}>
                  Modifica
                </button>
                {!self && (
                  <>
                    <DeleteButton
                      label="Reimposta password"
                      onConfirm={() =>
                        act(async () => {
                          const r = await api.resetPassword(u.id);
                          setSecret({ title: "Password reimpostata", user: u.username, password: r.password });
                        })
                      }
                    />
                    {u.two_factor && <DeleteButton label="Reimposta 2FA" onConfirm={() => act(() => api.resetTwoFactor(u.id))} />}
                    <DeleteButton onConfirm={() => act(() => api.deleteUser(u.id))} />
                  </>
                )}
              </>
            );
          }}
        />
      </div>

      <div className="card">
        <h2>Sicurezza</h2>
        <p className="lead small-lead">
          Il secondo fattore (codice dell’app di autenticazione) si attiva dal profilo di ciascuno. Qui puoi renderlo
          obbligatorio: chi non l’ha ancora configurata viene guidato a farlo al prossimo accesso.
        </p>
        <div className="inline">
          <select value={policy} onChange={(e) => setPolicy(e.target.value as TwoFactorPolicy)} aria-label="Criterio 2FA">
            {(Object.keys(POLICY_LABEL) as TwoFactorPolicy[]).map((k) => (
              <option key={k} value={k}>
                {POLICY_LABEL[k]}
              </option>
            ))}
          </select>
          <button
            className="primary"
            onClick={() =>
              act(async () => {
                await api.setPolicy(policy);
                setPolicyMsg("Criterio salvato.");
              })
            }
          >
            Salva
          </button>
        </div>
        {policyMsg && <div className="box good">{policyMsg}</div>}
      </div>

      {creating && (
        <UserModal
          scopes={scopes}
          onClose={() => setCreating(false)}
          onSaved={(u, password) => {
            setCreating(false);
            setSecret({ title: "Utente creato", user: u, password });
            load();
          }}
        />
      )}
      {editing && (
        <UserModal
          scopes={scopes}
          user={editing}
          self={editing.username === me.username}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            load();
          }}
        />
      )}
      {secret && (
        <Modal title={secret.title} onClose={() => setSecret(null)}>
          <p className="lead small-lead">
            Comunica questa password temporanea a <strong>{secret.user}</strong>: al primo accesso dovrà sceglierne una
            sua. Non verrà mostrata di nuovo.
          </p>
          <div className="secret">
            <code>{secret.password}</code>
            <button className="ghost small" onClick={() => void navigator.clipboard.writeText(secret.password).catch(() => undefined)}>
              Copia
            </button>
          </div>
          <div className="nav">
            <span />
            <button className="primary" onClick={() => setSecret(null)}>
              Fatto
            </button>
          </div>
        </Modal>
      )}
    </>
  );
}

function UserModal(props: {
  scopes: ScopeInfo[];
  user?: UserInfo;
  self?: boolean;
  onClose: () => void;
  onSaved: (username: string, password: string) => void;
}) {
  const { user } = props;
  const [username, setUsername] = useState(user?.username ?? "");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState(user?.role ?? "viewer");
  const [custom, setCustom] = useState<string[]>(user?.custom_scopes ?? []);
  const [disabled, setDisabled] = useState(user?.disabled ?? false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const toggle = (id: string) => setCustom((c) => (c.includes(id) ? c.filter((x) => x !== id) : [...c, id]));

  const save = async () => {
    setBusy(true);
    setErr(null);
    try {
      if (user) {
        await api.updateUser(user.id, { role, scopes: role === "custom" ? custom : undefined, disabled });
        props.onSaved(user.username, "");
      } else {
        const r = await api.createUser({ username, password: password || undefined, role, scopes: role === "custom" ? custom : undefined });
        props.onSaved(r.user.username, r.password);
      }
    } catch (e) {
      setErr((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <Modal title={user ? `Modifica ${user.username}` : "Nuovo utente"} onClose={props.onClose} wide>
      {!user && (
        <div className="row">
          <Field label="Nome utente" hint="Da 3 a 64 caratteri, senza spazi.">
            <input value={username} onChange={(e) => setUsername(e.target.value)} autoFocus autoComplete="off" spellCheck={false} />
          </Field>
          <Field label="Password temporanea" hint="Lasciala vuota per generarne una: l’utente dovrà cambiarla al primo accesso.">
            <input value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="off" spellCheck={false} />
          </Field>
        </div>
      )}
      <Field label="Ruolo" hint={ROLE_HELP[role]}>
        <select value={role} onChange={(e) => setRole(e.target.value)}>
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {ROLE_LABEL[r]}
            </option>
          ))}
        </select>
      </Field>
      {role === "custom" && (
        <fieldset className="scopes">
          <legend className="flabel">Permessi</legend>
          {props.scopes.map((s) => (
            <label key={s.id} className="check">
              <input type="checkbox" checked={custom.includes(s.id)} onChange={() => toggle(s.id)} />
              <span>
                <code>{s.id}</code>
                <span className="muted block small-text">{s.description}</span>
              </span>
            </label>
          ))}
          <p className="hint">La scrittura include la lettura della stessa risorsa.</p>
        </fieldset>
      )}
      {user && (
        <label className="check">
          <input type="checkbox" checked={disabled} disabled={props.self} onChange={(e) => setDisabled(e.target.checked)} />
          <span>
            Account disabilitato
            <span className="muted block small-text">{props.self ? "Non puoi disabilitare il tuo account." : "Non può più accedere e le sue sessioni si chiudono subito."}</span>
          </span>
        </label>
      )}
      {err && <div className="box bad">{err}</div>}
      <div className="nav">
        <button className="ghost" onClick={props.onClose}>
          Annulla
        </button>
        <button className="primary" onClick={save} disabled={busy || (!user && username.trim().length < 3)}>
          {busy ? "Salvo…" : user ? "Salva" : "Crea utente"}
        </button>
      </div>
    </Modal>
  );
}

function AuditTab() {
  const [rows, setRows] = useState<AuditEntry[]>([]);
  const [more, setMore] = useState(true);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const PAGE = 100;

  const load = useCallback(async (before?: string) => {
    setBusy(true);
    try {
      const r = await api.audit(PAGE, before);
      setRows((prev) => (before ? [...prev, ...r.entries] : r.entries));
      setMore(r.entries.length === PAGE);
      setErr(null);
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const columns: Column<AuditEntry>[] = [
    { key: "ts", header: "Quando", sort: (e) => e.ts, render: (e) => <span className="muted">{fmtTime(e.ts)}</span> },
    { key: "user", header: "Utente", sort: (e) => e.user, render: (e) => <strong>{e.user}</strong> },
    { key: "action", header: "Azione", sort: (e) => ACTION_LABEL[e.action] ?? e.action, render: (e) => ACTION_LABEL[e.action] ?? e.action },
    { key: "target", header: "Dettaglio", render: (e) => (e.target ? <code>{e.target}</code> : <span className="muted">—</span>) },
    {
      key: "ok",
      header: "Esito",
      sort: (e) => (e.ok ? 0 : 1),
      render: (e) => (e.ok ? <span className="badge good">Ok</span> : <span className="badge bad">Negato</span>),
    },
  ];

  return (
    <div className="card">
      <div className="head">
        <h2>Registro attività</h2>
        <button className="secondary small" onClick={() => void load()} disabled={busy}>
          Aggiorna
        </button>
      </div>
      {err && <div className="box bad">{err}</div>}
      <DataTable
        columns={columns}
        rows={rows}
        rowKey={(e) => `${e.ts}${e.user}${e.action}${e.target}`}
        empty="Ancora nessuna attività registrata."
        searchText={(e) => `${e.user} ${ACTION_LABEL[e.action] ?? e.action} ${e.target}`}
        searchPlaceholder="Cerca per utente, azione o dettaglio…"
        initialSort={{ key: "ts", dir: "desc" }}
      />
      {more && rows.length > 0 && (
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => void load(rows[rows.length - 1].ts)} disabled={busy}>
            {busy ? "Carico…" : "Carica altre"}
          </button>
        </div>
      )}
    </div>
  );
}
