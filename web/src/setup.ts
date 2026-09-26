import type { PanelState } from "./api";
import { tr } from "./i18n";

export interface SetupStep {
  id: "domain" | "bucket" | "route";
  done: boolean;
  title: string;
  text: string;
}

/** I tre passi per avere il primo file online. */
export function setupSteps(s: PanelState): SetupStep[] {
  const { domains, buckets, rules } = s.panel;
  return [
    {
      id: "domain",
      done: domains.some((d) => d.verified),
      title: tr("setup.domain.title"),
      text: tr("setup.domain.text"),
    },
    {
      id: "bucket",
      done: buckets.length > 0,
      title: tr("setup.bucket.title"),
      text: tr("setup.bucket.text"),
    },
    {
      id: "route",
      done: rules.length > 0,
      title: tr("setup.route.title"),
      text: tr("setup.route.text"),
    },
  ];
}

export const setupDone = (steps: SetupStep[]) => steps.filter((x) => x.done).length;
export const setupComplete = (steps: SetupStep[]) => steps.every((x) => x.done);

/** Chi non può scrivere domini, bucket e instradamenti non può completare la configurazione. */
export const canSetup = (can: (scope: string) => boolean) =>
  can("domains:write") && can("buckets:write") && can("routes:write");
