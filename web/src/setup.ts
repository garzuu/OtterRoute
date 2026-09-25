import type { PanelState } from "./api";

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
      title: "Aggiungi un dominio verificato",
      text: "Il sistema controlla che i record DNS puntino a questo nodo e che sia raggiungibile.",
    },
    {
      id: "bucket",
      done: buckets.length > 0,
      title: "Collega un bucket",
      text: "Endpoint e credenziali dello storage S3.",
    },
    {
      id: "route",
      done: rules.length > 0,
      title: "Crea un instradamento",
      text: "Dominio e prefisso verso un bucket e una cartella.",
    },
  ];
}

export const setupDone = (steps: SetupStep[]) => steps.filter((x) => x.done).length;
export const setupComplete = (steps: SetupStep[]) => steps.every((x) => x.done);

/** Chi non può scrivere domini, bucket e instradamenti non può completare la configurazione. */
export const canSetup = (can: (scope: string) => boolean) =>
  can("domains:write") && can("buckets:write") && can("routes:write");
