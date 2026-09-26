import { useCallback, useEffect, useState } from "react";
import { api, type AuditEntry, type ScopeInfo, type TwoFactorPolicy, type UserInfo } from "../api";
import { ROLE_HELP, ROLE_LABEL, useAuth } from "../auth";
import { DataTable, type Column } from "../DataTable";
import { it, rich, srv, tr, trn, type Key } from "../i18n";
import { DeleteButton, EmptyState, Field, Modal, Page, fmtTime } from "../ui";

const ROLES = ["admin", "operator", "viewer", "custom"];

const POLICY_LABEL: Record<TwoFactorPolicy, Key> = { off: "us.pOff", all: "us.pAll", managers: "us.pMgr" };

/** Etichetta di un'azione del registro; se non è tradotta si mostra il codice. */
const actionLabel = (a: string) => (`act.${a}` in it ? tr(`act.${a}` as Key) : a);

type Tab = "users" | "audit";

export function Users() {
  const [tab, setTab] = useState<Tab>("users");
  return (
    <Page title={tr("nav.users")} lead={tr("us.lead")}>
      <div className="toolbar">
        <div className="seg" role="tablist" aria-label={tr("us.section")}>
          <button role="tab" aria-selected={tab === "users"} className={tab === "users" ? "on" : ""} onClick={() => setTab("users")}>
            {tr("us.tabUsers")}
          </button>
          <button role="tab" aria-selected={tab === "audit"} className={tab === "audit" ? "on" : ""} onClick={() => setTab("audit")}>
            {tr("us.tabAudit")}
          </button>
        </div>
      </div>
      {tab === "users" ? <UsersTab /> : <AuditTab />}
    </Page>
  );
}

function scopeSummary(u: UserInfo) {
  return u.role === "custom" ? trn("us.permN", u.scopes.length) : (ROLE_LABEL[u.role] ?? u.role);
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
      header: tr("us.colUser"),
      sort: (u) => u.username.toLowerCase(),
      render: (u) => (
        <span>
          <strong>{u.username}</strong> {u.username === me.username && <span className="badge">{tr("us.you")}</span>}
        </span>
      ),
    },
    { key: "role", header: tr("us.colRole"), sort: (u) => u.role, render: (u) => scopeSummary(u) },
    {
      key: "tfa",
      header: "2FA",
      sort: (u) => (u.two_factor ? 1 : 0),
      render: (u) => (u.two_factor ? <span className="badge good">{tr("tfa.on")}</span> : <span className="muted">—</span>),
    },
    {
      key: "state",
      header: tr("bk2.colStatus"),
      sort: (u) => (u.disabled ? 2 : u.must_change_password ? 1 : 0),
      render: (u) =>
        u.disabled ? (
          <span className="badge bad">{tr("us.disabled")}</span>
        ) : u.must_change_password ? (
          <span className="badge warn">{tr("us.tempPw")}</span>
        ) : (
          <span className="badge good">{tr("us.active")}</span>
        ),
    },
    { key: "last", header: tr("us.colLast"), sort: (u) => u.last_login_at ?? "", render: (u) => <span className="muted">{fmtTime(u.last_login_at)}</span> },
  ];

  return (
    <>
      {err && <div className="box bad">{err}</div>}
      <div className="card">
        <div className="head">
          <h2>{tr("nav.users")}</h2>
          <button className="primary small" onClick={() => setCreating(true)}>
            {tr("us.newUser")}
          </button>
        </div>
        <DataTable
          columns={columns}
          rows={users}
          rowKey={(u) => u.id}
          empty={<EmptyState image="welcome" title={tr("us.none")} />}
          searchText={(u) => `${u.username} ${u.role}`}
          searchPlaceholder={tr("us.search")}
          initialSort={{ key: "user", dir: "asc" }}
          actions={(u) => {
            const self = u.username === me.username;
            return (
              <>
                <button className="secondary small" onClick={() => setEditing(u)}>
                  {tr("us.edit")}
                </button>
                {!self && (
                  <>
                    <DeleteButton
                      label={tr("us.resetPw")}
                      onConfirm={() =>
                        act(async () => {
                          const r = await api.resetPassword(u.id);
                          setSecret({ title: tr("us.pwReset"), user: u.username, password: r.password });
                        })
                      }
                    />
                    {u.two_factor && <DeleteButton label={tr("us.resetTfa")} onConfirm={() => act(() => api.resetTwoFactor(u.id))} />}
                    <DeleteButton onConfirm={() => act(() => api.deleteUser(u.id))} />
                  </>
                )}
              </>
            );
          }}
        />
      </div>

      <div className="card">
        <h2>{tr("us.security")}</h2>
        <p className="lead small-lead">
          {tr("us.secLead")}
        </p>
        <div className="inline">
          <select value={policy} onChange={(e) => setPolicy(e.target.value as TwoFactorPolicy)} aria-label={tr("us.policyAria")}>
            {(Object.keys(POLICY_LABEL) as TwoFactorPolicy[]).map((k) => (
              <option key={k} value={k}>
                {tr(POLICY_LABEL[k])}
              </option>
            ))}
          </select>
          <button
            className="primary"
            onClick={() =>
              act(async () => {
                await api.setPolicy(policy);
                setPolicyMsg(tr("us.policySaved"));
              })
            }
          >
            {tr("common.save")}
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
            setSecret({ title: tr("us.created"), user: u, password });
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
            {rich(tr("us.secretLead", { user: `<b>${secret.user}</b>` }))}
          </p>
          <div className="secret">
            <code>{secret.password}</code>
            <button className="ghost small" onClick={() => void navigator.clipboard.writeText(secret.password).catch(() => undefined)}>
              {tr("common.copy")}
            </button>
          </div>
          <div className="nav">
            <span />
            <button className="primary" onClick={() => setSecret(null)}>
              {tr("us.done")}
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
    <Modal title={user ? tr("us.editUser", { user: user.username }) : tr("us.newUser")} onClose={props.onClose} wide>
      {!user && (
        <div className="row">
          <Field label={tr("us.username")} hint={tr("us.usernameHint")}>
            <input value={username} onChange={(e) => setUsername(e.target.value)} autoFocus autoComplete="off" spellCheck={false} />
          </Field>
          <Field label={tr("us.tempField")} hint={tr("us.tempHint")}>
            <input value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="off" spellCheck={false} />
          </Field>
        </div>
      )}
      <Field label={tr("us.roleField")} hint={ROLE_HELP[role]}>
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
          <legend className="flabel">{tr("us.perms")}</legend>
          {props.scopes.map((s) => (
            <label key={s.id} className="check">
              <input type="checkbox" checked={custom.includes(s.id)} onChange={() => toggle(s.id)} />
              <span>
                <code>{s.id}</code>
                <span className="muted block small-text">{srv(s.description)}</span>
              </span>
            </label>
          ))}
          <p className="hint">{tr("us.writeIncludesRead")}</p>
        </fieldset>
      )}
      {user && (
        <label className="check">
          <input type="checkbox" checked={disabled} disabled={props.self} onChange={(e) => setDisabled(e.target.checked)} />
          <span>
            {tr("us.accDisabled")}
            <span className="muted block small-text">{props.self ? tr("us.noSelfDisable") : tr("us.disabledNote")}</span>
          </span>
        </label>
      )}
      {err && <div className="box bad">{err}</div>}
      <div className="nav">
        <button className="ghost" onClick={props.onClose}>
          {tr("common.cancel")}
        </button>
        <button className="primary" onClick={save} disabled={busy || (!user && username.trim().length < 3)}>
          {busy ? tr("common.saving") : user ? tr("common.save") : tr("us.create")}
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
    { key: "ts", header: tr("us.colWhen"), sort: (e) => e.ts, render: (e) => <span className="muted">{fmtTime(e.ts)}</span> },
    { key: "user", header: tr("us.colUser"), sort: (e) => e.user, render: (e) => <strong>{e.user}</strong> },
    { key: "action", header: tr("us.colAction"), sort: (e) => actionLabel(e.action), render: (e) => actionLabel(e.action) },
    { key: "target", header: tr("us.colDetail"), render: (e) => (e.target ? <code>{e.target}</code> : <span className="muted">—</span>) },
    {
      key: "ok",
      header: tr("us.colResult"),
      sort: (e) => (e.ok ? 0 : 1),
      render: (e) => (e.ok ? <span className="badge good">{tr("us.ok")}</span> : <span className="badge bad">{tr("us.denied")}</span>),
    },
  ];

  return (
    <div className="card">
      <div className="head">
        <h2>{tr("us.log")}</h2>
        <button className="secondary small" onClick={() => void load()} disabled={busy}>
          {tr("us.refresh")}
        </button>
      </div>
      {err && <div className="box bad">{err}</div>}
      <DataTable
        columns={columns}
        rows={rows}
        rowKey={(e) => `${e.ts}${e.user}${e.action}${e.target}`}
        empty={tr("us.noActivity")}
        searchText={(e) => `${e.user} ${actionLabel(e.action)} ${e.target}`}
        searchPlaceholder={tr("us.searchAudit")}
        initialSort={{ key: "ts", dir: "desc" }}
      />
      {more && rows.length > 0 && (
        <div className="nav">
          <span />
          <button className="secondary" onClick={() => void load(rows[rows.length - 1].ts)} disabled={busy}>
            {busy ? tr("us.loadingMore") : tr("us.loadMore")}
          </button>
        </div>
      )}
    </div>
  );
}
