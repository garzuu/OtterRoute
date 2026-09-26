import { createContext, Fragment, useContext, useMemo, useState, type ReactNode } from "react";
import { en } from "./en";
import { it, type Key } from "./it";

export type Lang = "it" | "en";
export { it };
export type { Key };
type Params = Record<string, string | number>;

const DICTS: Record<Lang, Record<Key, string>> = { it, en };
const LOCALES: Record<Lang, string> = { it: "it-IT", en: "en-GB" };
const STORE = "otr.lang";

/** La lingua salvata; altrimenti quella del browser; altrimenti l'italiano. */
function initial(): Lang {
  try {
    const saved = localStorage.getItem(STORE);
    if (saved === "it" || saved === "en") return saved;
  } catch {
    /* archivio non disponibile */
  }
  return navigator.language?.toLowerCase().startsWith("en") ? "en" : "it";
}

let current: Lang = initial();
if (typeof document !== "undefined") document.documentElement.lang = current;

/** Formato di date e numeri della lingua in uso (per le funzioni fuori dai componenti). */
export const loc = () => LOCALES[current];

function fill(msg: string, p?: Params): string {
  return p ? msg.replace(/\{(\w+)\}/g, (m, k: string) => (k in p ? String(p[k]) : m)) : msg;
}

/** Traduzione senza hook, per le funzioni di utilità: `tr("common.never")`. */
export function tr(key: Key, p?: Params): string {
  return fill(DICTS[current][key] ?? it[key] ?? key, p);
}

/** Testo con <b>…</b> e <code>…</code> in nodi React, senza HTML grezzo. */
export function rich(text: string): ReactNode[] {
  return text.split(/(<b>.*?<\/b>|<code>.*?<\/code>)/g).map((part, i) => {
    const m = /^<(b|code)>(.*)<\/\1>$/.exec(part);
    if (!m) return part;
    return m[1] === "b" ? <strong key={i}>{m[2]}</strong> : <code key={i}>{m[2]}</code>;
  });
}

let serverKeys: Map<string, Key> | null = null;

/** Messaggio arrivato dal server (scritto in italiano): se è noto lo traduce, altrimenti lo lascia com'è. */
export function srv(text: string): string {
  if (!serverKeys) {
    serverKeys = new Map();
    for (const k of Object.keys(it) as Key[]) if (k.startsWith("srv.")) serverKeys.set(it[k], k);
  }
  const k = serverKeys.get(text);
  return k ? tr(k) : text;
}

/** Plurali: cerca `<chiave>.one` / `<chiave>.other` secondo la lingua; `{n}` è il numero. */
export function trn(key: string, n: number, p?: Params): string {
  const cat = new Intl.PluralRules(LOCALES[current]).select(n) === "one" ? "one" : "other";
  const k = `${key}.${cat}` as Key;
  return tr(k in it ? k : (`${key}.other` as Key), { ...p, n });
}

interface Ctx {
  lang: Lang;
  setLang: (l: Lang) => void;
}
const LangCtx = createContext<Ctx>({ lang: "it", setLang: () => undefined });

/** Cambiare lingua rimonta l'albero (`key`): è un gesto raro e così ogni testo si aggiorna. */
export function LangProvider(props: { children: ReactNode }) {
  const [lang, set] = useState<Lang>(current);
  const value = useMemo<Ctx>(
    () => ({
      lang,
      setLang: (l) => {
        current = l;
        try {
          localStorage.setItem(STORE, l);
        } catch {
          /* archivio non disponibile */
        }
        document.documentElement.lang = l;
        set(l);
      },
    }),
    [lang],
  );
  return (
    <LangCtx.Provider value={value}>
      <Fragment key={lang}>{props.children}</Fragment>
    </LangCtx.Provider>
  );
}

export const useLang = () => useContext(LangCtx);
export const useT = () => tr;

/** Voce del menu utente: scelta della lingua. */
export function LangSwitch() {
  const { lang, setLang } = useLang();
  return (
    <div className="pop-item" role="group" aria-label={tr("lang.label")} style={{ cursor: "default", justifyContent: "space-between" }}>
      <span className="muted">{tr("lang.label")}</span>
      <span style={{ display: "flex", gap: 6 }}>
        {(["it", "en"] as const).map((l) => (
          <button key={l} className={l === lang ? "badge good" : "badge"} style={{ cursor: "pointer", border: 0 }} aria-pressed={l === lang} onClick={() => l !== lang && setLang(l)}>
            {l.toUpperCase()}
          </button>
        ))}
      </span>
    </div>
  );
}

/** Scelta della lingua per le schermate senza menu (accesso). */
export function LangToggle() {
  const { lang, setLang } = useLang();
  return (
    <span className="muted small-text" role="group" aria-label={tr("lang.label")}>
      {(["it", "en"] as const).map((l, i) => (
        <span key={l}>
          {i > 0 && " · "}
          <button className="link" style={{ fontWeight: l === lang ? 700 : 400 }} aria-pressed={l === lang} onClick={() => l !== lang && setLang(l)}>
            {l.toUpperCase()}
          </button>
        </span>
      ))}
    </span>
  );
}
