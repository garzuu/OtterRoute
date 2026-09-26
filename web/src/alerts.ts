import { domainStatus, type PanelState } from "./api";
import type { PageId } from "./Dashboard";
import { tr } from "./i18n";
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
      title: tr("al.setup.title", { done: setupDone(steps), total: steps.length }),
      text: tr("al.setup.text", { title: next.title, text: next.text }),
      page: "overview",
      action: "wizard",
    });
  if (s.hand_managed)
    out.push({
      id: "hand-managed",
      level: "warn",
      title: tr("al.hand.title"),
      text: tr("al.hand.text"),
      page: "overview",
    });
  for (const ch of s.notify_failing ?? [])
    out.push({
      id: `n:${ch}`,
      level: "warn",
      title: tr("al.notify.title", { ch: ch === "email" ? "email" : "Telegram" }),
      text: tr("al.notify.text"),
      page: "notifications",
    });
  const u = s.update;
  if (u?.available && u.latest && can("settings:write"))
    out.push({
      id: `u:${u.latest.version}`,
      level: "info",
      title: tr("al.update.title", { v: u.latest.version }),
      text: tr("al.update.text", { cur: u.current }),
      page: "settings",
    });
  if (u?.updated_from)
    out.push({
      id: `u:done:${u.current}`,
      level: "info",
      title: tr("al.updated.title", { v: u.current }),
      text: tr("al.updated.text", { from: u.updated_from }),
      page: "settings",
    });
  if (u?.rollback)
    out.push({
      id: `u:rollback:${u.rollback.to}`,
      level: "warn",
      title: tr("al.rollback.title", { v: u.rollback.to }),
      text: tr("al.rollback.text", { reason: u.rollback.reason }),
      page: "settings",
    });
  const now = Date.now() / 1000;
  if (s.panel.settings.acme.enabled)
    for (const c of can("domains:read") ? s.certs : []) {
      if (c.status === "expired" || (c.status === "expiring" && c.not_after && c.not_after > now))
        out.push({
          id: `c:${c.host}`,
          level: c.status === "expired" ? "error" : "warn",
          title: tr(c.status === "expired" ? "al.cert.expired" : "al.cert.expiring", { host: c.host }),
          text: tr(c.status === "expired" ? "al.cert.expiredText" : "al.cert.expiringText"),
          page: "domains",
        });
      else if (c.status === "error")
        out.push({ id: `c:${c.host}`, level: "warn", title: tr("al.cert.error", { host: c.host }), text: c.error?.message ?? "", page: "domains" });
    }
  for (const d of can("domains:read") ? s.panel.domains : []) {
    const st = domainStatus(d);
    if (st === "verified") continue;
    // cosa non va: il DNS non risolve, oppure il dominio risolve ma il nodo non risponde
    const failing = d.stages.find((x) => x.status === "fail")?.id;
    const title = tr(failing === "reach" ? "al.dom.reach" : "al.dom.dns", { host: d.host });
    out.push({
      id: `d:${d.host}`,
      // era valido e ora non lo è più: errore; mai verificato: attesa della propagazione
      level: st === "pending" ? "warn" : "error",
      title,
      text: d.message,
      page: "domains",
    });
  }
  for (const b of can("buckets:read") ? s.panel.buckets : []) {
    const c = b.check;
    if (!c) {
      out.push({ id: `b:${b.id}`, level: "warn", title: tr("al.bkt.unverified", { name: b.name }), text: tr("al.bkt.unverifiedText"), page: "buckets" });
    } else if (c.outcome === "unreachable") {
      out.push({ id: `b:${b.id}`, level: "error", title: tr("al.bkt.unreachable", { name: b.name }), text: c.message, page: "buckets" });
    } else if (c.outcome === "auth") {
      out.push({ id: `b:${b.id}`, level: "error", title: tr("al.bkt.auth", { name: b.name }), text: c.message, page: "buckets" });
    } else if (!c.ok) {
      out.push({ id: `b:${b.id}`, level: "error", title: tr("al.bkt.error", { name: b.name }), text: c.message, page: "buckets" });
    } else if (c.outcome !== "found") {
      out.push({ id: `b:${b.id}`, level: "warn", title: tr("al.bkt.notfound", { name: b.name }), text: c.message, page: "buckets" });
    }
  }
  const rank = { error: 0, info: 1, warn: 2 } as const;
  return out.sort((a, b) => rank[a.level] - rank[b.level]);
}
