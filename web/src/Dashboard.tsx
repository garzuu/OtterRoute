import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ApiError, api, type PanelState } from "./api";
import { buildAlerts } from "./alerts";
import { useAuth } from "./auth";
import { IconBell, IconBuckets, IconDomains, IconOverview, IconRoutes, IconSettings, IconUsers } from "./icons";
import { Topbar } from "./Topbar";
import { Notifications } from "./pages/Notifications";
import { Wizard } from "./Wizard";
import type { Alert } from "./alerts";
import { Overview } from "./pages/Overview";
import { Domains } from "./pages/Domains";
import { Buckets } from "./pages/Buckets";
import { Routes } from "./pages/Routes";
import { Settings } from "./pages/Settings";
import { Users } from "./pages/Users";
import { Profile } from "./pages/Profile";
import { canSetup } from "./setup";

export type PageId = "overview" | "domains" | "buckets" | "routes" | "notifications" | "settings" | "users" | "profile";

/** Voci della sidebar; `scope` è ciò che serve per vederle. */
const NAV: { id: PageId; label: string; Icon: typeof IconOverview; scope?: string }[] = [
  { id: "overview", label: "Panoramica", Icon: IconOverview },
  { id: "domains", label: "Domini", Icon: IconDomains, scope: "domains:read" },
  { id: "buckets", label: "Bucket", Icon: IconBuckets, scope: "buckets:read" },
  { id: "routes", label: "Instradamenti", Icon: IconRoutes, scope: "routes:read" },
  { id: "users", label: "Utenti", Icon: IconUsers, scope: "users:manage" },
  { id: "notifications", label: "Notifiche", Icon: IconBell, scope: "notifications:manage" },
  { id: "settings", label: "Impostazioni", Icon: IconSettings },
];

const PAGES: PageId[] = ["overview", "domains", "buckets", "routes", "notifications", "settings", "users", "profile"];

/** La pagina indicata nell'indirizzo, solo se l'utente può vederla. */
const pageFromHash = (can: (s: string) => boolean): PageId => {
  const h = window.location.hash.replace(/^#\/?/, "") as PageId;
  if (!PAGES.includes(h)) return "overview";
  const scope = NAV.find((n) => n.id === h)?.scope;
  return scope && !can(scope) ? "overview" : h;
};

export function Dashboard(props: { username: string; onLoggedOut: () => void; onUnauthorized: () => void }) {
  const { can, user } = useAuth();
  const [page, setPage] = useState<PageId>(() => pageFromHash(can));
  const [state, setState] = useState<PanelState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem("otr.sidebar") === "collapsed";
    } catch {
      return false;
    }
  });
  const [drawer, setDrawer] = useState(false);
  const [wizard, setWizard] = useState(false);
  const autoOpened = useRef(false);
  const { onUnauthorized } = props;

  useEffect(() => {
    try {
      localStorage.setItem("otr.sidebar", collapsed ? "collapsed" : "open");
    } catch {
      /* preferenza solo comoda: senza storage si riparte aperta */
    }
  }, [collapsed]);

  const go = (p: PageId) => {
    window.location.hash = `/${p}`;
    setPage(p);
  };

  useEffect(() => {
    const on = () => setPage(pageFromHash(can));
    window.addEventListener("hashchange", on);
    return () => window.removeEventListener("hashchange", on);
  }, []);

  const refresh = useCallback(async () => {
    try {
      setState(await api.panel());
      setError(null);
    } catch (e) {
      if (e instanceof ApiError && e.status === 401) onUnauthorized();
      else setError((e as Error).message);
    }
  }, [onUnauthorized]);

  useEffect(() => {
    void refresh();
    const t = setInterval(() => void refresh(), 15000);
    return () => clearInterval(t);
  }, [refresh]);

  const logout = async () => {
    await api.logout().catch(() => undefined);
    props.onLoggedOut();
  };

  const WIZARD_KEY = "otr.wizard.dismissed";
  const closeWizard = () => {
    setWizard(false);
    try {
      localStorage.setItem(WIZARD_KEY, "1");
    } catch {
      /* preferenza solo comoda */
    }
  };
  // prima visita su un nodo vuoto: la configurazione guidata si apre da sola, una volta
  useEffect(() => {
    if (!state || autoOpened.current) return;
    autoOpened.current = true;
    const empty = !state.panel.domains.length && !state.panel.buckets.length && !state.panel.rules.length;
    let dismissed = false;
    try {
      dismissed = localStorage.getItem(WIZARD_KEY) === "1";
    } catch {
      /* senza storage si riapre a ogni visita */
    }
    if (empty && !dismissed && canSetup(can)) setWizard(true);
  }, [state, can]);
  const onAlert = (a: Alert) => {
    if (a.action === "wizard") setWizard(true);
    else navigate(a.page);
  };

  const alerts = useMemo(() => (state ? buildAlerts(state, can) : []), [state, can]);

  const isMobile = () => window.matchMedia("(max-width: 760px)").matches;
  const toggleSidebar = () => {
    if (isMobile()) setDrawer((v) => !v);
    else setCollapsed((v) => !v);
  };
  const navigate = (p: PageId) => {
    setDrawer(false);
    go(p);
  };

  return (
    <div className={`app${collapsed ? " collapsed" : ""}${drawer ? " drawer" : ""}`}>
      <aside className="side">
        <div className="brand">
          <img className="logo-img" src="/logo-tile.png" alt="" width={32} height={32} />
          <span className="brand-name">OtterRoute</span>
        </div>
        <nav aria-label="Sezioni">
          {NAV.filter((n) => !n.scope || can(n.scope)).map((n) => (
            <button
              key={n.id}
              className={n.id === page ? "nav-item on" : "nav-item"}
              onClick={() => navigate(n.id)}
              title={n.label}
              aria-label={n.label}
              aria-current={n.id === page ? "page" : undefined}
            >
              <n.Icon className="ico" />
              <span className="label">{n.label}</span>
            </button>
          ))}
        </nav>
      </aside>
      {drawer && <div className="scrim" onClick={() => setDrawer(false)} aria-hidden />}

      <div className="content">
        <Topbar
          page={page}
          username={props.username}
          alerts={alerts}
          role={user.role}
          onProfile={() => navigate("profile")}
          onToggleSidebar={toggleSidebar}
          onAlert={onAlert}
          onLogout={logout}
        />
        <main className="main">
        {error && <div className="box bad">{error}</div>}
        {state && (
          <>
            {page === "overview" && <Overview state={state} onWizard={() => setWizard(true)} />}
            {page === "domains" && <Domains state={state} refresh={refresh} />}
            {page === "buckets" && <Buckets state={state} refresh={refresh} />}
            {page === "routes" && <Routes state={state} refresh={refresh} go={go} />}
            {page === "notifications" && can("notifications:manage") && <Notifications />}
            {page === "settings" && <Settings state={state} refresh={refresh} />}
            {page === "users" && can("users:manage") && <Users />}
            {page === "profile" && <Profile />}
          </>
        )}
        </main>
      </div>
      {wizard && state && <Wizard state={state} refresh={refresh} go={navigate} onClose={closeWizard} />}
    </div>
  );
}
