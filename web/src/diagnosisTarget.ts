/** Indirizzo da diagnosticare, passato da un'altra pagina (Domini, Instradamenti) alla pagina Diagnosi. */
let pending: string | null = null;

export const setDiagnosisTarget = (url: string | null) => {
  pending = url;
};

export const takeDiagnosisTarget = (): string | null => {
  const u = pending;
  pending = null;
  return u;
};
