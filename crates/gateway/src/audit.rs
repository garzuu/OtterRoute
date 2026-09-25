//! Registro delle attività: chi ha fatto cosa e quando. File append-only
//! (`state/audit.jsonl`, una riga JSON per evento), con rotazione a 5 MB.
//! L'utente corrente viaggia in un task-local impostato da `admin::handle`,
//! così gli handler esistenti non cambiano firma (stesso schema di `metrics`).

use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

const MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub ts: String,
    pub user: String,
    pub action: String,
    #[serde(default)]
    pub target: String,
    pub ok: bool,
}

pub struct Audit {
    path: PathBuf,
    lock: Mutex<()>,
}

impl Audit {
    pub fn new(path: PathBuf) -> Self {
        Audit {
            path,
            lock: Mutex::new(()),
        }
    }

    fn rotated(&self) -> PathBuf {
        self.path.with_extension("jsonl.1")
    }

    pub fn append(&self, e: &Entry) {
        let _g = self.lock.lock().unwrap();
        if std::fs::metadata(&self.path).is_ok_and(|m| m.len() >= MAX_BYTES) {
            let _ = std::fs::rename(&self.path, self.rotated());
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        match opts.open(&self.path) {
            Ok(mut f) => {
                if let Ok(line) = serde_json::to_string(e) {
                    let _ = writeln!(f, "{line}");
                }
            }
            Err(err) => tracing::warn!(error = %err, "impossibile scrivere il registro attività"),
        }
    }

    /// Le voci più recenti per prime; `before` è un timestamp ISO: solo le più vecchie.
    pub fn tail(&self, limit: usize, before: Option<&str>) -> Vec<Entry> {
        let _g = self.lock.lock().unwrap();
        let mut out: Vec<Entry> = Vec::new();
        for file in [&self.path, &self.rotated()] {
            let Ok(text) = std::fs::read_to_string(file) else {
                continue;
            };
            let mut chunk: Vec<Entry> = text
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .filter(|e: &Entry| before.is_none_or(|b| e.ts.as_str() < b))
                .collect();
            chunk.reverse();
            out.extend(chunk);
            if out.len() >= limit {
                break;
            }
        }
        out.truncate(limit);
        out
    }
}

static AUDIT: OnceLock<Audit> = OnceLock::new();

pub fn init(state_dir: &Path) {
    let _ = AUDIT.set(Audit::new(state_dir.join("audit.jsonl")));
}

pub fn global() -> Option<&'static Audit> {
    AUDIT.get()
}

tokio::task_local! {
    static USER: String;
}

/// Esegue `f` attribuendo le azioni a `user`.
pub async fn scope<F: Future>(user: String, f: F) -> F::Output {
    USER.scope(user, f).await
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn log_as(user: &str, action: &str, target: &str, ok: bool) {
    if let Some(a) = AUDIT.get() {
        a.append(&Entry {
            ts: now_iso(),
            user: user.to_owned(),
            action: action.to_owned(),
            target: target.to_owned(),
            ok,
        });
    }
}

/// Azione riuscita dell'utente della richiesta corrente.
pub fn log(action: &str, target: &str) {
    let user = USER.try_with(|u| u.clone()).unwrap_or_else(|_| "-".into());
    log_as(&user, action, target, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(n: u32) -> Entry {
        Entry {
            ts: format!("2026-09-25T10:00:{n:02}.000Z"),
            user: "admin".into(),
            action: "domain.add".into(),
            target: format!("d{n}.it"),
            ok: true,
        }
    }

    #[test]
    fn append_tail_and_before() {
        let dir = tempfile::tempdir().unwrap();
        let a = Audit::new(dir.path().join("audit.jsonl"));
        for n in 0..10 {
            a.append(&entry(n));
        }
        let t = a.tail(3, None);
        assert_eq!(
            t.iter().map(|e| e.target.as_str()).collect::<Vec<_>>(),
            ["d9.it", "d8.it", "d7.it"]
        );
        // paginazione: solo le voci prima del timestamp indicato
        let older = a.tail(3, Some(&t[2].ts));
        assert_eq!(older[0].target, "d6.it");
        // una riga rovinata non rompe la lettura
        std::fs::OpenOptions::new()
            .append(true)
            .open(dir.path().join("audit.jsonl"))
            .unwrap()
            .write_all(b"non json\n")
            .unwrap();
        assert_eq!(a.tail(100, None).len(), 10);
    }

    #[test]
    fn rotation_keeps_history() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let a = Audit::new(path.clone());
        a.append(&entry(1));
        // simula un file pieno
        let big = std::fs::File::options().append(true).open(&path).unwrap();
        big.set_len(MAX_BYTES + 1).unwrap();
        a.append(&entry(2));
        assert!(dir.path().join("audit.jsonl.1").exists());
        let all = a.tail(10, None);
        assert_eq!(all[0].target, "d2.it");
        assert!(all.iter().any(|e| e.target == "d1.it"));
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let a = Audit::new(dir.path().join("audit.jsonl"));
        a.append(&entry(1));
        let mode = std::fs::metadata(dir.path().join("audit.jsonl"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
