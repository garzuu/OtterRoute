import { domainStatus, type PanelState } from "./api";
import type { PageId } from "./Dashboard";
import { canSetup, setupDone, setupSteps } from "./setup";

export interface Alert {
  id: string;
  level: "error" | "warn" | "info";
  title: string;
  text: string;
  page: PageId;
  /** al posto della navigazione apre la configurazione guidata */
  action?: "wizard";
}

/** Tutto ciò che richiede attenzione, con gli errori per primi. */
export function buildAlerts(s: PanelState, can: (scope: string) => boolean): Alert[] {
  const out: Alert[] = [];
  const steps = setupSteps(s);
  const next = steps.find((x) => !x.done);
  if (next && canSetup(can))
    out.push({
      id: "setup",
      level: "info",
      title: `Completa la configurazione · ${setupDone(steps)} di ${steps.length}`,
      text: `Prossimo passo: ${next.title}. ${next.text}`,
      page: "overview",
      action: "wizard",
    });
  if (s.hand_managed)
    out.push({
      id: "hand-managed",
      level: "warn",
      title: "Configurazione gestita a mano",
      text: "Il nodo usa un config.yaml scritto a mano: il pannello non lo modifica.",
      page: "overview",
    });
  for (const ch of s.notify_failing ?? [])
    out.push({
      id: `n:${ch}`,
      level: "warn",
      title: `Notifiche ${ch === "email" ? "email" : "Telegram"} non recapitate`,
      text: "L’ultimo invio è fallito: controlla le impostazioni e premi «Invia prova».",
      page: "notifications",
    });
  for (const d of can("domains:read") ? s.panel.domains : []) {
    const st = domainStatus(d);
    if (st === "verified") continue;
    // cosa non va: il DNS non risolve, oppure il dominio risolve ma il nodo non risponde
    const failing = d.stages.find((x) => x.status === "fail")?.id;
    const title = failing === "reach" ? "Il nodo non risponde attraverso il dominio" : "Il dominio non risolve";
    out.push({
      id: `d:${d.host}`,
      // era valido e ora non lo è più: errore; mai verificato: attesa della propagazione
      level: st === "pending" ? "warn" : "error",
      title: `${title} · ${d.host}`,
      text: d.message,
      page: "domains",
    });
  }
  for (const b of can("buckets:read") ? s.panel.buckets : []) {
    const c = b.check;
    if (!c) {
      out.push({ id: `b:${b.id}`, level: "warn", title: `Bucket non verificato · ${b.name}`, text: "Non è ancora stata provata la lettura.", page: "buckets" });
    } else if (c.outcome === "unreachable") {
      out.push({ id: `b:${b.id}`, level: "error", title: `Bucket non raggiungibile · ${b.name}`, text: c.message, page: "buckets" });
    } else if (c.outcome === "auth") {
      out.push({ id: `b:${b.id}`, level: "error", title: `Credenziali rifiutate · ${b.name}`, text: c.message, page: "buckets" });
    } else if (!c.ok) {
      out.push({ id: `b:${b.id}`, level: "error", title: `Errore sul bucket · ${b.name}`, text: c.message, page: "buckets" });
    } else if (c.outcome !== "found") {
      out.push({ id: `b:${b.id}`, level: "warn", title: `File di prova non trovato · ${b.name}`, text: c.message, page: "buckets" });
    }
  }
  const rank = { error: 0, info: 1, warn: 2 } as const;
  return out.sort((a, b) => rank[a.level] - rank[b.level]);
}
