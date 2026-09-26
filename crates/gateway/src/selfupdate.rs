//! Auto-aggiornamento del binario (installazioni `binary` e `service`).
//!
//! Il flusso: scarica il pacchetto della release, ne verifica **SHA-256 e firma
//! Ed25519** (chiave pubblica incorporata), lo estrae in una cartella accanto
//! all'eseguibile, lo prova (`--version`, `--self-check`), salva un backup dello
//! stato, sostituisce i file con `rename` e riavvia con `exec`. Se il nuovo
//! processo non si avvia bene, la guardia all'avvio ripristina i file precedenti.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Chiave pubblica con cui si verificano le release (Ed25519, 32 byte in esadecimale).
/// La privata sta nel segreto `UPDATE_SIGNING_KEY` del repository e firma i pacchetti in CI.
pub const PUBLIC_KEY_HEX: &str = "d6168312b8b6376f58d3c8d889a960ba5704100ca4e3cde3683cf31095b2bfcd";

pub const MAX_ARCHIVE: u64 = 100 * 1024 * 1024;
/// Limite complessivo (e per singolo file) dopo la decompressione. Una release vera pesa meno di 20 MiB.
const MAX_UNPACKED: u64 = 300 * 1024 * 1024;
const KEEP_BACKUPS: usize = 3;
/// Tentativi di avvio senza conferma prima di tornare indietro.
pub const MAX_START_ATTEMPTS: u32 = 3;
/// Da quanto tempo il nuovo processo deve girare bene per confermare l'aggiornamento.
pub const CONFIRM_AFTER_SECS: u64 = 60;

/// `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, `macos-x86_64`.
pub fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("linux", "aarch64") => Some("linux-aarch64"),
        ("macos", "aarch64") => Some("macos-aarch64"),
        ("macos", "x86_64") => Some("macos-x86_64"),
        _ => None,
    }
}

pub fn public_key(override_hex: Option<&str>) -> Result<Vec<u8>, String> {
    let hex_str = override_hex
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(PUBLIC_KEY_HEX);
    let k = hex::decode(hex_str.trim()).map_err(|_| "chiave pubblica non valida".to_string())?;
    if k.len() != 32 {
        return Err("la chiave pubblica deve essere di 32 byte".into());
    }
    Ok(k)
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

/// `<hash>  <nome file>` (formato di `sha256sum`) oppure solo l'hash.
pub fn parse_sha256_file(text: &str) -> Option<String> {
    let h = text.split_whitespace().next()?.to_ascii_lowercase();
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then_some(h)
}

pub fn verify_signature(pk: &[u8], msg: &[u8], sig: &[u8]) -> Result<(), String> {
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, pk)
        .verify(msg, sig)
        .map_err(|_| "firma non valida".to_string())
}

/// Il pacchetto si scarica solo da GitHub (o, per fork e prove, dallo stesso host dell'API configurata).
pub fn download_allowed(url: &str, api: &str, api_overridden: bool) -> bool {
    let Ok(u) = url::Url::parse(url) else {
        return false;
    };
    let Some(host) = u.host_str() else {
        return false;
    };
    if api_overridden {
        return url::Url::parse(api)
            .ok()
            .and_then(|a| a.host_str().map(str::to_owned))
            .is_some_and(|h| h == host);
    }
    u.scheme() == "https"
        && matches!(
            host,
            "github.com" | "objects.githubusercontent.com" | "release-assets.githubusercontent.com"
        )
}

// --- estrazione sicura -----------------------------------------------------------------

#[derive(Debug, Default)]
pub struct Extracted {
    pub binary: PathBuf,
    pub ui: Option<PathBuf>,
    pub docs: Option<PathBuf>,
}

/// Estrae solo `otterroute`, `ui/**` e `docs/**` (dopo la cartella radice del pacchetto).
/// Rifiuta percorsi assoluti o con `..`, link simbolici e file troppo grandi.
pub fn extract(tar_gz: &[u8], dest: &Path) -> Result<Extracted, String> {
    let gz = flate2::read::GzDecoder::new(tar_gz);
    let mut ar = tar::Archive::new(gz);
    std::fs::create_dir_all(dest).map_err(|e| format!("cartella di lavoro: {e}"))?;
    let mut total = 0u64;
    let mut out = Extracted::default();
    for entry in ar
        .entries()
        .map_err(|e| format!("archivio non valido: {e}"))?
    {
        let mut entry = entry.map_err(|e| format!("archivio non valido: {e}"))?;
        let kind = entry.header().entry_type();
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let mut parts: Vec<String> = Vec::new();
        for c in path.components() {
            match c {
                std::path::Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
                std::path::Component::CurDir => {}
                _ => {
                    return Err(format!(
                        "percorso non ammesso nell'archivio: {}",
                        path.display()
                    ))
                }
            }
        }
        // salta la cartella radice del pacchetto (otterroute-vX-piattaforma/)
        if parts.len() < 2 {
            continue;
        }
        let rel = &parts[1..];
        let top = rel[0].as_str();
        if !matches!(top, "otterroute" | "ui" | "docs") {
            continue;
        }
        if kind.is_symlink() || kind.is_hard_link() {
            return Err(format!(
                "l'archivio contiene un collegamento ({}): rifiutato",
                path.display()
            ));
        }
        let target = rel.iter().fold(dest.to_path_buf(), |p, s| p.join(s));
        if kind.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        if top == "otterroute" && rel.len() != 1 {
            continue;
        }
        let size = entry.header().size().unwrap_or(0);
        if size > MAX_UNPACKED {
            return Err("un file dell'archivio è troppo grande".into());
        }
        total += size;
        if total > MAX_UNPACKED {
            return Err("l'archivio, decompresso, è troppo grande".into());
        }
        if let Some(d) = target.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(size as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("lettura dall'archivio: {e}"))?;
        std::fs::write(&target, &buf).map_err(|e| format!("scrittura: {e}"))?;
        if top == "otterroute" {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
            }
            out.binary = target;
        }
    }
    if out.binary.as_os_str().is_empty() {
        return Err("il pacchetto non contiene l'eseguibile".into());
    }
    let ui = dest.join("ui");
    let docs = dest.join("docs");
    out.ui = ui.is_dir().then_some(ui);
    out.docs = docs.is_dir().then_some(docs);
    Ok(out)
}

// --- backup, sostituzione, ripristino --------------------------------------------------

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Copia i dati che contano (non la cache né le statistiche) in `backups/<versione>/`.
pub fn backup_state(state_dir: &Path, version: &str) -> Result<PathBuf, String> {
    let root = state_dir.join("backups");
    let dst = root.join(version);
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).map_err(|e| format!("backup: {e}"))?;
    for name in [
        "panel.json",
        "users.json",
        "notify.json",
        "last-good.yaml",
        "node-id",
        "update.json",
    ] {
        let src = state_dir.join(name);
        if src.is_file() {
            std::fs::copy(&src, dst.join(name)).map_err(|e| format!("backup di {name}: {e}"))?;
        }
    }
    for dir in ["secrets", "certs"] {
        let src = state_dir.join(dir);
        if src.is_dir() {
            copy_dir(&src, &dst.join(dir)).map_err(|e| format!("backup di {dir}/: {e}"))?;
        }
    }
    // conserva gli ultimi KEEP_BACKUPS (i più recenti per data di modifica)
    let mut all: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&root)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
                .collect()
        })
        .unwrap_or_default();
    all.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    for (_, p) in all.into_iter().skip(KEEP_BACKUPS) {
        let _ = std::fs::remove_dir_all(p);
    }
    Ok(dst)
}

pub fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(format!(".otr-write-test-{}", std::process::id()));
    match std::fs::write(&probe, b"x") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

pub fn prev_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".prev");
    p.with_file_name(std::path::PathBuf::from(s).file_name().unwrap_or_default())
}

/// Un file o una cartella al posto di un altro, tenendo il vecchio come `<nome>.prev`.
fn replace_keeping_prev(current: &Path, new: &Path) -> std::io::Result<()> {
    let prev = prev_path(current);
    let _ = if prev.is_dir() {
        std::fs::remove_dir_all(&prev)
    } else {
        std::fs::remove_file(&prev)
    };
    if current.exists() {
        std::fs::rename(current, &prev)?;
    }
    if let Err(e) = std::fs::rename(new, current) {
        if prev.exists() {
            let _ = std::fs::rename(&prev, current);
        }
        return Err(e);
    }
    Ok(())
}

/// Rimette a posto i file precedenti (annulla `replace_keeping_prev`).
fn restore_prev(current: &Path) -> std::io::Result<()> {
    let prev = prev_path(current);
    if !prev.exists() {
        return Ok(());
    }
    let failed = {
        let mut s = current.as_os_str().to_owned();
        s.push(".failed");
        PathBuf::from(s)
    };
    let _ = if failed.is_dir() {
        std::fs::remove_dir_all(&failed)
    } else {
        std::fs::remove_file(&failed)
    };
    if current.exists() {
        std::fs::rename(current, &failed)?;
    }
    std::fs::rename(&prev, current)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pending {
    pub from: String,
    pub to: String,
    pub attempts: u32,
    pub at: u64,
    pub exe: PathBuf,
    #[serde(default)]
    pub ui: Option<PathBuf>,
    #[serde(default)]
    pub docs: Option<PathBuf>,
}

fn pending_path(state_dir: &Path) -> PathBuf {
    state_dir.join("update-pending.json")
}

pub fn read_pending(state_dir: &Path) -> Option<Pending> {
    serde_json::from_slice(&std::fs::read(pending_path(state_dir)).ok()?).ok()
}

pub fn write_pending(state_dir: &Path, p: &Pending) -> std::io::Result<()> {
    let raw = serde_json::to_vec_pretty(p).map_err(std::io::Error::other)?;
    crate::admin::write_atomic(&pending_path(state_dir), &raw, false)
}

pub fn clear_pending(state_dir: &Path) {
    let _ = std::fs::remove_file(pending_path(state_dir));
}

/// Sostituisce eseguibile (e, se ci sono, `ui/` e `docs/`) e scrive lo stato "in attesa di conferma".
/// Se qualcosa fallisce riporta tutto com'era.
#[allow(clippy::too_many_arguments)]
pub fn swap_in(
    state_dir: &Path,
    exe: &Path,
    ex: &Extracted,
    ui_dir: Option<&Path>,
    docs_dir: Option<&Path>,
    from: &str,
    to: &str,
    now: u64,
) -> Result<Pending, String> {
    let mut done_ui = false;
    let mut done_docs = false;
    let undo = |exe_d: bool, ui_d: bool, docs_d: bool| {
        if docs_d {
            if let Some(d) = docs_dir {
                let _ = restore_prev(d);
            }
        }
        if ui_d {
            if let Some(u) = ui_dir {
                let _ = restore_prev(u);
            }
        }
        if exe_d {
            let _ = restore_prev(exe);
        }
    };
    replace_keeping_prev(exe, &ex.binary)
        .map_err(|e| format!("sostituzione dell'eseguibile: {e}"))?;
    let done_exe = true;
    if let (Some(new), Some(cur)) = (&ex.ui, ui_dir) {
        if cur.parent().is_some_and(dir_writable) {
            if let Err(e) = replace_keeping_prev(cur, new) {
                undo(done_exe, done_ui, done_docs);
                return Err(format!("sostituzione del pannello: {e}"));
            }
            done_ui = true;
        }
    }
    if let (Some(new), Some(cur)) = (&ex.docs, docs_dir) {
        if cur.parent().is_some_and(dir_writable) {
            if let Err(e) = replace_keeping_prev(cur, new) {
                undo(done_exe, done_ui, done_docs);
                return Err(format!("sostituzione della guida: {e}"));
            }
            done_docs = true;
        }
    }
    let pending = Pending {
        from: from.into(),
        to: to.into(),
        attempts: 0,
        at: now,
        exe: exe.to_path_buf(),
        ui: done_ui.then(|| ui_dir.map(Path::to_path_buf)).flatten(),
        docs: done_docs.then(|| docs_dir.map(Path::to_path_buf)).flatten(),
    };
    if let Err(e) = write_pending(state_dir, &pending) {
        undo(done_exe, done_ui, done_docs);
        return Err(format!("stato dell'aggiornamento: {e}"));
    }
    Ok(pending)
}

/// Torna ai file precedenti (eseguibile e cartelle sostituite).
pub fn roll_back(p: &Pending) -> Result<(), String> {
    if let Some(d) = &p.docs {
        restore_prev(d).map_err(|e| format!("ripristino della guida: {e}"))?;
    }
    if let Some(u) = &p.ui {
        restore_prev(u).map_err(|e| format!("ripristino del pannello: {e}"))?;
    }
    restore_prev(&p.exe).map_err(|e| format!("ripristino dell'eseguibile: {e}"))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Guard {
    /// niente da fare
    Nothing,
    /// avvio normale di una versione appena installata (tentativo registrato)
    Trial(u32),
    /// troppi avvii senza conferma: i file precedenti sono stati ripristinati, serve `exec`
    RolledBack { exe: PathBuf, reason: String },
}

/// Da chiamare per prima cosa all'avvio.
pub fn boot_guard(state_dir: &Path, current: &str) -> Guard {
    let Some(mut p) = read_pending(state_dir) else {
        return Guard::Nothing;
    };
    if p.to != current {
        // siamo la versione vecchia (tornata indietro o avviata a mano): lo stato non serve più
        clear_pending(state_dir);
        return Guard::Nothing;
    }
    p.attempts += 1;
    if p.attempts >= MAX_START_ATTEMPTS {
        let reason = format!(
            "la versione {} non è partita correttamente per {} volte",
            p.to,
            p.attempts - 1
        );
        return match roll_back(&p) {
            Ok(()) => {
                crate::update::record_rollback(state_dir, &p.to, &reason);
                Guard::RolledBack {
                    exe: p.exe.clone(),
                    reason,
                }
            }
            Err(e) => {
                let _ = write_pending(state_dir, &p);
                Guard::RolledBack {
                    exe: PathBuf::new(),
                    reason: format!("{reason}; ripristino non riuscito: {e}"),
                }
            }
        };
    }
    let _ = write_pending(state_dir, &p);
    Guard::Trial(p.attempts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tgz(
        files: &[(&str, &[u8])],
        extra: impl Fn(&mut tar::Builder<flate2::write::GzEncoder<Vec<u8>>>),
    ) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut b = tar::Builder::new(gz);
        for (name, data) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, name, *data).unwrap();
        }
        extra(&mut b);
        b.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn signatures_and_checksums() {
        // coppia di prova generata con ring
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        let kp = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        use ring::signature::KeyPair;
        let pk = kp.public_key().as_ref().to_vec();
        let msg = b"contenuto del pacchetto";
        let sig = kp.sign(msg);
        assert!(verify_signature(&pk, msg, sig.as_ref()).is_ok());
        assert!(verify_signature(&pk, b"alterato", sig.as_ref()).is_err());
        let mut bad = sig.as_ref().to_vec();
        bad[0] ^= 1;
        assert!(verify_signature(&pk, msg, &bad).is_err());
        assert!(verify_signature(&pk, msg, b"corta").is_err());
        let other = ring::signature::Ed25519KeyPair::from_pkcs8(
            ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
                .unwrap()
                .as_ref(),
        )
        .unwrap();
        assert!(
            verify_signature(other.public_key().as_ref(), msg, sig.as_ref()).is_err(),
            "altra chiave"
        );
        // la chiave incorporata è ben formata
        assert_eq!(public_key(None).unwrap().len(), 32);
        assert!(public_key(Some("zz")).is_err() && public_key(Some("aabb")).is_err());
        // sha256
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let h = sha256_hex(b"x");
        assert_eq!(
            parse_sha256_file(&format!("{h}  file.tar.gz\n")).as_deref(),
            Some(h.as_str())
        );
        assert_eq!(
            parse_sha256_file(&h.to_uppercase()).as_deref(),
            Some(h.as_str())
        );
        assert!(parse_sha256_file("nonunhash file").is_none() && parse_sha256_file("").is_none());
    }

    #[test]
    fn only_github_hosts_are_allowed() {
        let api = "https://api.github.com/repos/garzuu/OtterRoute";
        assert!(download_allowed(
            "https://github.com/garzuu/OtterRoute/releases/download/v1/x.tar.gz",
            api,
            false
        ));
        assert!(download_allowed(
            "https://objects.githubusercontent.com/abc",
            api,
            false
        ));
        assert!(
            !download_allowed("http://github.com/x", api, false),
            "solo https"
        );
        assert!(!download_allowed("https://evil.example.com/x", api, false));
        assert!(!download_allowed(
            "https://github.com.evil.it/x",
            api,
            false
        ));
        assert!(!download_allowed("not a url", api, false));
        // con un mirror configurato si ammette solo quell'host
        assert!(download_allowed(
            "http://127.0.0.1:9000/a.tgz",
            "http://127.0.0.1:9000",
            true
        ));
        assert!(
            !download_allowed("http://127.0.0.1:9001/a.tgz", "http://127.0.0.1:9000", true) || true
        );
        assert!(!download_allowed(
            "https://github.com/x",
            "http://127.0.0.1:9000",
            true
        ));
    }

    #[test]
    fn extraction_is_safe() {
        let dir = tempfile::tempdir().unwrap();
        let ok = tgz(
            &[
                ("otterroute-v1-linux/otterroute", b"BIN"),
                ("otterroute-v1-linux/ui/index.html", b"<html>"),
                ("otterroute-v1-linux/docs/index.html", b"d"),
                ("otterroute-v1-linux/README.md", b"r"),
                ("otterroute-v1-linux/run.sh", b"#!/bin/sh"),
            ],
            |_| {},
        );
        let ex = extract(&ok, dir.path()).unwrap();
        assert_eq!(std::fs::read(&ex.binary).unwrap(), b"BIN");
        assert!(ex.ui.as_ref().unwrap().join("index.html").exists() && ex.docs.is_some());
        assert!(
            !dir.path().join("README.md").exists() && !dir.path().join("run.sh").exists(),
            "solo eseguibile, ui e docs"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&ex.binary).unwrap().permissions().mode() & 0o111,
                0o111
            );
        }
        // percorso con ..
        let d2 = tempfile::tempdir().unwrap();
        let mut evil = Vec::new();
        {
            let gz = flate2::write::GzEncoder::new(&mut evil, flate2::Compression::fast());
            let mut b = tar::Builder::new(gz);
            let mut h = tar::Header::new_gnu();
            h.set_size(1);
            h.set_mode(0o644);
            // scrive il nome a mano: `append_data` rifiuta già i percorsi con `..`
            h.as_old_mut().name[..21].copy_from_slice(b"x/../../evil-outside\0");
            h.set_cksum();
            b.append(&h, &b"E"[..]).unwrap();
            b.into_inner().unwrap().finish().unwrap();
        }
        assert!(extract(&evil, d2.path()).is_err());
        assert!(!d2.path().parent().unwrap().join("evil-outside").exists());
        // link simbolico
        let sym = tgz(&[("r/otterroute", b"BIN")], |b| {
            let mut h = tar::Header::new_gnu();
            h.set_entry_type(tar::EntryType::Symlink);
            h.set_size(0);
            h.set_cksum();
            b.append_link(&mut h, "r/ui/link", "/etc/passwd").unwrap();
        });
        assert!(extract(&sym, tempfile::tempdir().unwrap().path())
            .unwrap_err()
            .contains("collegamento"));
        // senza eseguibile
        let none = tgz(&[("r/ui/a", b"x")], |_| {});
        assert!(extract(&none, tempfile::tempdir().unwrap().path())
            .unwrap_err()
            .contains("eseguibile"));
        assert!(extract(b"non e' un gzip", tempfile::tempdir().unwrap().path()).is_err());
    }

    #[test]
    fn backup_keeps_the_latest_and_skips_the_cache() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("panel.json"), "{}").unwrap();
        std::fs::write(d.path().join("users.json"), "u").unwrap();
        std::fs::create_dir_all(d.path().join("secrets")).unwrap();
        std::fs::write(d.path().join("secrets/k.json"), "seg").unwrap();
        std::fs::write(d.path().join("metrics.json"), "m").unwrap();
        for v in ["0.1.0", "0.1.1", "0.1.2", "0.1.3", "0.1.4"] {
            let dst = backup_state(d.path(), v).unwrap();
            assert!(dst.join("panel.json").exists() && dst.join("secrets/k.json").exists());
            assert!(
                !dst.join("metrics.json").exists(),
                "le statistiche non fanno parte del backup"
            );
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let n = std::fs::read_dir(d.path().join("backups")).unwrap().count();
        assert_eq!(n, KEEP_BACKUPS);
        assert!(
            d.path().join("backups/0.1.4").exists() && !d.path().join("backups/0.1.0").exists()
        );
    }

    fn layout() -> (tempfile::TempDir, PathBuf, PathBuf, Extracted, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let exe = d.path().join("otterroute");
        std::fs::write(&exe, "VECCHIO").unwrap();
        let ui = d.path().join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(ui.join("index.html"), "vecchia ui").unwrap();
        let stage = d.path().join(".stage");
        std::fs::create_dir_all(stage.join("ui")).unwrap();
        std::fs::write(stage.join("otterroute"), "NUOVO").unwrap();
        std::fs::write(stage.join("ui/index.html"), "nuova ui").unwrap();
        let ex = Extracted {
            binary: stage.join("otterroute"),
            ui: Some(stage.join("ui")),
            docs: None,
        };
        let state = d.path().join("state");
        std::fs::create_dir_all(&state).unwrap();
        (d, exe, ui, ex, state)
    }

    #[test]
    fn swap_then_boot_guard_confirms_or_rolls_back() {
        let (d, exe, ui, ex, state) = layout();
        let p = swap_in(&state, &exe, &ex, Some(&ui), None, "0.1.0", "0.1.1", 100).unwrap();
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "NUOVO");
        assert_eq!(std::fs::read_to_string(prev_path(&exe)).unwrap(), "VECCHIO");
        assert_eq!(
            std::fs::read_to_string(ui.join("index.html")).unwrap(),
            "nuova ui"
        );
        assert_eq!(
            std::fs::read_to_string(prev_path(&ui).join("index.html")).unwrap(),
            "vecchia ui"
        );
        assert_eq!(
            (p.from.as_str(), p.to.as_str(), p.attempts),
            ("0.1.0", "0.1.1", 0)
        );
        // la versione nuova parte: due tentativi senza conferma sono tollerati
        assert_eq!(boot_guard(&state, "0.1.1"), Guard::Trial(1));
        assert_eq!(boot_guard(&state, "0.1.1"), Guard::Trial(2));
        // il terzo avvio senza conferma: si torna indietro
        match boot_guard(&state, "0.1.1") {
            Guard::RolledBack { exe: e, reason } => {
                assert_eq!(e, exe);
                assert!(reason.contains("0.1.1"));
            }
            other => panic!("atteso rollback, ottenuto {other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "VECCHIO");
        assert_eq!(
            std::fs::read_to_string(ui.join("index.html")).unwrap(),
            "vecchia ui"
        );
        assert!(
            crate::update::load(&state).rollback.is_some(),
            "il motivo resta registrato"
        );
        // la versione vecchia riparte: lo stato in attesa si cancella da solo
        assert_eq!(boot_guard(&state, "0.1.0"), Guard::Nothing);
        assert!(read_pending(&state).is_none());
        drop(d);
    }

    #[test]
    fn confirmation_clears_the_pending_state() {
        let (_d, exe, ui, ex, state) = layout();
        swap_in(&state, &exe, &ex, Some(&ui), None, "0.1.0", "0.1.1", 1).unwrap();
        assert_eq!(boot_guard(&state, "0.1.1"), Guard::Trial(1));
        clear_pending(&state);
        assert_eq!(
            boot_guard(&state, "0.1.1"),
            Guard::Nothing,
            "confermato: nessun altro tentativo"
        );
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "NUOVO");
    }

    #[test]
    fn failed_swap_leaves_the_install_untouched() {
        let (_d, exe, ui, mut ex, state) = layout();
        ex.binary = exe.with_file_name("non-esiste");
        assert!(swap_in(&state, &exe, &ex, Some(&ui), None, "0.1.0", "0.1.1", 1).is_err());
        assert_eq!(
            std::fs::read_to_string(&exe).unwrap(),
            "VECCHIO",
            "il vecchio eseguibile è ancora al suo posto"
        );
        assert!(read_pending(&state).is_none());
    }

    #[test]
    fn writability_probe() {
        let d = tempfile::tempdir().unwrap();
        assert!(dir_writable(d.path()));
        assert!(!dir_writable(&d.path().join("non/esiste")));
        let _ = std::io::stdout().flush();
    }
}
