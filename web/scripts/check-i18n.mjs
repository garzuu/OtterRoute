// Controlla che it.ts e en.ts abbiano le stesse chiavi e gli stessi segnaposto {nome} e tag <b>/<code>.
import { readFileSync } from "node:fs";

const load = (f) => {
  const out = new Map();
  for (const line of readFileSync(new URL(`../src/i18n/${f}`, import.meta.url), "utf8").split("\n")) {
    const m = /^\s*"([^"]+)":\s*("(?:[^"\\]|\\.)*"),?\s*$/.exec(line);
    if (m) out.set(m[1], JSON.parse(m[2]));
  }
  return out;
};
const sig = (s) => [...s.matchAll(/\{(\w+)\}|<(\/?(?:b|code))>/g)].map((m) => m[1] ?? `<${m[2]}>`).sort().join(",");

const it = load("it.ts");
const en = load("en.ts");
let bad = 0;
for (const [k, v] of it) {
  if (!en.has(k)) { console.error(`manca in en: ${k}`); bad++; continue; }
  if (sig(v) !== sig(en.get(k))) { console.error(`segnaposto diversi in ${k}: [${sig(v)}] / [${sig(en.get(k))}]`); bad++; }
  if (en.get(k).trim() === "") { console.error(`vuota in en: ${k}`); bad++; }
}
for (const k of en.keys()) if (!it.has(k)) { console.error(`in più in en: ${k}`); bad++; }
console.log(`${it.size} chiavi, ${bad} problemi`);
process.exit(bad ? 1 : 0);
