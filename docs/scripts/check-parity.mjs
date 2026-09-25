#!/usr/bin/env node
// Controlla che la guida italiana e quella inglese siano allineate:
//  - ogni pagina elencata in .vitepress/pages.ts esiste in entrambe le lingue;
//  - nessun file in più in una sola lingua;
//  - le pagine hanno lo stesso numero di sezioni (## ) e di blocchi di codice;
//  - ogni pagina inglese dichiara `source_commit` (la versione italiana da cui deriva).
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pagesSrc = readFileSync(join(root, ".vitepress/pages.ts"), "utf8");
const slugs = [...pagesSrc.matchAll(/slug:\s*"([^"]+)"/g)].map((m) => m[1]);

const errors = [];
const warnings = [];

const walk = (dir) =>
  existsSync(dir)
    ? readdirSync(dir).flatMap((f) => {
        const p = join(dir, f);
        return statSync(p).isDirectory() ? walk(p) : [p];
      })
    : [];
const filesOf = (lang) => new Set(walk(join(root, lang)).filter((f) => f.endsWith(".md")).map((f) => relative(join(root, lang), f)));

const it = filesOf("it");
const en = filesOf("en");
const expected = new Set(["index.md", ...slugs.map((s) => `${s}.md`)]);

for (const f of expected) {
  if (!it.has(f)) errors.push(`manca in italiano: ${f}`);
  if (!en.has(f)) errors.push(`manca in inglese: ${f}`);
}
for (const f of it) if (!expected.has(f)) errors.push(`pagina italiana non elencata in pages.ts: ${f}`);
for (const f of en) if (!expected.has(f)) errors.push(`pagina inglese non elencata in pages.ts: ${f}`);

const shape = (text) => {
  const body = text.replace(/^---[\s\S]*?---\n/, "");
  let inCode = false;
  let h2 = 0;
  let code = 0;
  for (const line of body.split("\n")) {
    if (line.startsWith("```")) {
      if (!inCode) code++;
      inCode = !inCode;
    } else if (!inCode && /^## /.test(line)) h2++;
  }
  return { h2, code };
};

for (const f of expected) {
  if (!it.has(f) || !en.has(f)) continue;
  const a = readFileSync(join(root, "it", f), "utf8");
  const b = readFileSync(join(root, "en", f), "utf8");
  const sa = shape(a);
  const sb = shape(b);
  if (sa.h2 !== sb.h2) errors.push(`${f}: sezioni diverse (it ${sa.h2}, en ${sb.h2})`);
  if (sa.code !== sb.code) errors.push(`${f}: blocchi di codice diversi (it ${sa.code}, en ${sb.code})`);
  if (!/^source_commit:\s*\S+/m.test(b.split("---")[1] ?? "")) warnings.push(`${f}: la pagina inglese non dichiara source_commit`);
}

for (const w of warnings) console.warn(`avviso: ${w}`);
if (errors.length) {
  for (const e of errors) console.error(`errore: ${e}`);
  console.error(`\n${errors.length} problemi di parità IT/EN`);
  process.exit(1);
}
console.log(`parità IT/EN ok (${expected.size} pagine${warnings.length ? `, ${warnings.length} avvisi` : ""})`);
