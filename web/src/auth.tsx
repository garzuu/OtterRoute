import { createContext, useContext, type ReactNode } from "react";
import type { SessionUser } from "./api";

export const ROLE_LABEL: Record<string, string> = {
  admin: "Amministratore",
  operator: "Operatore",
  viewer: "Sola lettura",
  custom: "Personalizzato",
};

export const ROLE_HELP: Record<string, string> = {
  admin: "Può fare tutto, compresa la gestione di utenti e sicurezza.",
  operator: "Legge e modifica domini, bucket e instradamenti; vede le statistiche. Niente impostazioni né utenti.",
  viewer: "Vede domini, bucket, instradamenti e statistiche, senza poter modificare nulla.",
  custom: "Scegli gli scope uno per uno.",
};

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
export const needScope = (scope: string) => `Serve lo scope ${scope}`;
