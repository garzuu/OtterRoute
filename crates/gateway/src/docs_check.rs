//! La documentazione in `docs/` non deve mentire: esempi di configurazione,
//! variabili d'ambiente, scope e metriche sono confrontati con il codice.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn docs_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs")
}

/// Pagine Markdown di tutte le lingue presenti (`docs/it`, `docs/en`).
fn pages() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    for lang in ["it", "en"] {
        walk(&docs_root().join(lang), &mut out);
    }
    out
}

fn read(rel: &str) -> Vec<(String, String)> {
    ["it", "en"]
        .iter()
        .filter_map(|l| {
            let p = docs_root().join(l).join(rel);
            std::fs::read_to_string(&p)
                .ok()
                .map(|s| (format!("{l}/{rel}"), s))
        })
        .collect()
}

/// Blocchi ```yaml che iniziano con `version:`: sono configurazioni di OtterRoute.
fn config_blocks(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in md.lines() {
        match (&mut cur, line.trim_start()) {
            (None, l) if l.starts_with("```yaml") => cur = Some(String::new()),
            (Some(_), "```") => {
                let b = cur.take().unwrap();
                if b.starts_with("version:") {
                    out.push(b);
                }
            }
            (Some(b), _) => {
                b.push_str(line);
                b.push('\n');
            }
            _ => {}
        }
    }
    out
}

#[test]
fn config_examples_in_docs_are_valid() {
    let mut n = 0;
    for p in pages() {
        let md = std::fs::read_to_string(&p).unwrap();
        for block in config_blocks(&md) {
            let rc: crate::config::RawConfig = serde_yaml::from_str(&block)
                .unwrap_or_else(|e| panic!("{}: YAML non valido: {e}", p.display()));
            crate::config::validate(rc, &|_| Some("chiave-di-prova".into()))
                .unwrap_or_else(|e| panic!("{}: configurazione non valida: {e}", p.display()));
            n += 1;
        }
    }
    assert!(n > 0, "nessun esempio di configurazione trovato in docs/");
}

fn tokens<'a>(text: &'a str, prefix: &str) -> BTreeSet<&'a str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
        .filter(|t| t.starts_with(prefix))
        .collect()
}

#[test]
fn environment_reference_matches_flags() {
    let main = include_str!("main.rs");
    let in_code = tokens(main, "OTR_");
    assert!(!in_code.is_empty());
    let pages = read("reference/environment.md");
    assert!(!pages.is_empty(), "manca reference/environment.md");
    for (name, md) in pages {
        let in_doc = tokens(&md, "OTR_");
        for v in &in_code {
            assert!(in_doc.contains(v), "{name}: manca la variabile {v}");
        }
        for v in &in_doc {
            assert!(in_code.contains(v), "{name}: {v} non esiste nel codice");
        }
    }
}

#[test]
fn scopes_reference_matches_code() {
    let all: BTreeSet<&str> = crate::users::SCOPES.iter().map(|(k, _)| *k).collect();
    let pages = read("reference/scopes.md");
    assert!(!pages.is_empty(), "manca reference/scopes.md");
    for (name, md) in pages {
        for s in &all {
            assert!(md.contains(&format!("`{s}`")), "{name}: manca lo scope {s}");
        }
        let scope_like = |t: &&str| {
            t.contains(':')
                && !t.starts_with("x:")
                && (t.ends_with(":read") || t.ends_with(":write") || t.ends_with(":manage"))
        };
        for t in tokens(&md, "").into_iter().filter(scope_like) {
            assert!(all.contains(t), "{name}: scope inesistente {t}");
        }
    }
}

#[test]
fn metrics_reference_matches_code() {
    let src = concat!(include_str!("metrics.rs"), include_str!("metrics_extra.rs"));
    let in_code = tokens(src, "otterroute_");
    for (name, md) in read("reference/metrics.md") {
        for m in tokens(&md, "otterroute_") {
            let base = m
                .trim_end_matches("_bucket")
                .trim_end_matches("_sum")
                .trim_end_matches("_count");
            assert!(
                in_code.contains(m) || in_code.contains(base),
                "{name}: metrica inesistente {m}"
            );
        }
    }
}
