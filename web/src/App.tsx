import { useCallback, useEffect, useState } from "react";
import { api, type Session } from "./api";
import { AuthScreen } from "./AuthScreen";
import { Dashboard } from "./Dashboard";
import { ForcedPassword, ForcedTwoFactor } from "./Forced";
import { AuthProvider } from "./auth";
import { useT } from "./i18n";
import { setDocsBase } from "./ui";

export function App() {
  const t = useT();
  const [session, setSession] = useState<Session | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api
      .session()
      .then((s) => {
        setDocsBase(s.docs_url);
        setSession(s);
        setError(null);
      })
      .catch((e: Error) => setError(e.message));
  }, []);

  useEffect(load, [load]);

  if (error)
    return (
      <div className="shell">
        <div className="card">
          <h1>{t("app.unreachable")}</h1>
          <p className="lead">{error}</p>
          <button className="primary" onClick={load}>
            {t("common.retry")}
          </button>
        </div>
      </div>
    );
  if (!session) return null;

  if (!session.authenticated)
    return <AuthScreen mode={session.setup_required ? "setup" : "login"} onDone={load} />;

  const user = session.user;
  if (!user) return null;
  const logout = () => void api.logout().catch(() => undefined).then(load);
  if (user.requirement === "change_password") return <ForcedPassword onDone={load} onLogout={logout} />;
  if (user.requirement === "setup_2fa") return <ForcedTwoFactor onDone={load} onLogout={logout} />;

  return (
    <AuthProvider user={user} reloadSession={load}>
      <Dashboard
        username={user.username}
        onLoggedOut={load}
        // una risposta 401 (sessione scaduta) riporta al login
        onUnauthorized={load}
      />
    </AuthProvider>
  );
}

