import { createContext, useContext, type ReactNode } from "react";
import type { SessionUser } from "./api";
import { it, tr, type Key } from "./i18n";

/** Le etichette si leggono al momento dell'uso, così seguono la lingua scelta. */
const dyn = (prefix: string): Record<string, string> =>
  new Proxy({} as Record<string, string>, {
    get: (_, k) => (typeof k === "string" && `${prefix}.${k}` in it ? tr(`${prefix}.${k}` as Key) : undefined),
  });

export const ROLE_LABEL = dyn("role");

export const ROLE_HELP = dyn("roleHelp");

interface Ctx {
  user: SessionUser;
  /** L'utente può fare questa azione? (gli scope arrivano dal server) */
  can: (scope: string) => boolean;
  reloadSession: () => void;
}

const AuthContext = createContext<Ctx | null>(null);

export function AuthProvider(props: { user: SessionUser; reloadSession: () => void; children: ReactNode }) {
  const scopes = new Set(props.user.scopes);
  const value: Ctx = { user: props.user, can: (s) => scopes.has(s), reloadSession: props.reloadSession };
  return <AuthContext.Provider value={value}>{props.children}</AuthContext.Provider>;
}

export function useAuth(): Ctx {
  const c = useContext(AuthContext);
  if (!c) throw new Error("useAuth fuori da AuthProvider");
  return c;
}

/** Testo per i pulsanti disabilitati per mancanza di permesso. */
export const needScope = (scope: string) => tr("auth.needScope", { scope });
