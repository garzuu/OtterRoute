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
// ogni messaggio statico del server (bad("…"), error(…, "…"), Err("…")) deve avere una chiave srv.* con lo stesso testo
import { readdirSync } from "node:fs";
const srvIt = new Set([...it].filter(([k]) => k.startsWith("srv.")).map(([, v]) => v));
const dir = new URL("../../crates/gateway/src/", import.meta.url);
const found = new Set();
for (const f of readdirSync(dir).filter((x) => x.endsWith(".rs"))) {
  const src = readFileSync(new URL(f, dir), "utf8");
  for (const m of src.matchAll(/\b(?:bad|error)\((?:StatusCode::[A-Z_]+, )?"([^"\\]+)"/g)) found.add(m[1]);
  if (["users.rs", "auth.rs", "admin_users.rs", "totp.rs", "admin.rs", "notify.rs"].includes(f))
    for (const m of src.matchAll(/Err\((?:String::from\()?"([^"\\]+)"/g)) found.add(m[1]);
}
const SKIP = new Set(["not found"]);
for (const m of found) if (!SKIP.has(m) && !srvIt.has(m)) { console.error(`messaggio del server senza traduzione (srv.*): ${m}`); bad++; }
console.log(`${it.size} chiavi, ${found.size} messaggi del server, ${bad} problemi`);
process.exit(bad ? 1 : 0);
