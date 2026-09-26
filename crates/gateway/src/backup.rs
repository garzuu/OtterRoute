//! Backup e ripristino dello stato del nodo in un unico file cifrato con una frase segreta.
//!
//! Formato: `OTRBK1\0` · salt (16) · nonce (12) · AES-256-GCM(tar.gz). La chiave deriva dalla
//! frase con Argon2id. Il tar contiene `manifest.json` e `files/<percorso relativo>`.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use argon2::Argon2;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8] = b"OTRBK1\0";
const FORMAT: u32 = 1;
pub const MIN_PASSPHRASE: usize = 12;
/// dimensione massima dell'archivio caricato e di quanto si decomprime
pub const MAX_ARCHIVE: usize = 64 * 1024 * 1024;
const MAX_FILES: usize = 4000;
const TOP_FILES: [&str; 4] = ["panel.json", "users.json", "notify.json", "node-id"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub version: String,
    pub created_at: u64,
    pub node_id: Option<String>,
    pub files: Vec<String>,
}

#[derive(Debug)]
pub struct Opened {
    pub manifest: Manifest,
    pub files: Vec<(String, Vec<u8>)>,
}

fn derive(pass: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(pass.as_bytes(), salt, &mut key)
        .map_err(|e| format!("derivazione della chiave: {e}"))?;
    Ok(key)
}

fn cipher(key: &[u8; 32]) -> Result<LessSafeKey, String> {
    UnboundKey::new(&AES_256_GCM, key)
        .map(LessSafeKey::new)
        .map_err(|_| "chiave non valida".to_string())
}

/// I percorsi (relativi allo stato) che entrano nel backup.
fn collect(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = TOP_FILES
        .iter()
        .filter(|n| dir.join(n).is_file())
        .map(|n| n.to_string())
        .collect();
    for top in ["secrets", "certs"] {
        walk(&dir.join(top), top, &mut out);
    }
    out.sort();
    out
}

fn walk(path: &Path, rel: &str, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(path) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let r = format!("{rel}/{name}");
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&e.path(), &r, out),
            Ok(t) if t.is_file() && !name.ends_with(".tmp") => out.push(r),
            _ => {}
        }
    }
}

pub fn create(dir: &Path, pass: &str, now: u64) -> Result<Vec<u8>, String> {
    if pass.chars().count() < MIN_PASSPHRASE {
        return Err(format!(
            "la frase segreta deve avere almeno {MIN_PASSPHRASE} caratteri"
        ));
    }
    let files = collect(dir);
    let manifest = Manifest {
        format: FORMAT,
        version: crate::update::CURRENT.to_string(),
        created_at: now,
        node_id: std::fs::read_to_string(dir.join("node-id"))
            .ok()
            .map(|s| s.trim().to_string()),
        files: files.clone(),
    };
    let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut ar = tar::Builder::new(gz);
    let add = |ar: &mut tar::Builder<_>, name: &str, data: &[u8]| -> Result<(), String> {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o600);
        h.set_mtime(now);
        h.set_cksum();
        ar.append_data(&mut h, name, data)
            .map_err(|e| format!("archivio: {e}"))
    };
    add(
        &mut ar,
        "manifest.json",
        &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    )?;
    for f in &files {
        let data = std::fs::read(dir.join(f)).map_err(|e| format!("lettura di {f}: {e}"))?;
        add(&mut ar, &format!("files/{f}"), &data)?;
    }
    let mut plain = ar
        .into_inner()
        .and_then(|g| g.finish())
        .map_err(|e| format!("archivio: {e}"))?;

    let rng = SystemRandom::new();
    let (mut salt, mut nonce) = ([0u8; 16], [0u8; 12]);
    rng.fill(&mut salt)
        .map_err(|_| "casualità non disponibile")?;
    rng.fill(&mut nonce)
        .map_err(|_| "casualità non disponibile")?;
    let key = cipher(&derive(pass, &salt)?)?;
    let mut aad = MAGIC.to_vec();
    aad.extend_from_slice(&salt);
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(&aad),
        &mut plain,
    )
    .map_err(|_| "cifratura non riuscita")?;
    let mut out = aad;
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&plain);
    Ok(out)
}

/// Un percorso interno all'archivio ammesso? Solo i file che il backup può contenere.
fn allowed(rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\\'))
    {
        return false;
    }
    match parts.as_slice() {
        [f] => TOP_FILES.contains(f),
        ["secrets", _] => true,
        ["certs", _, _] => true,
        _ => false,
    }
}

pub fn open(data: &[u8], pass: &str) -> Result<Opened, String> {
    let head = MAGIC.len() + 16 + 12;
    if data.len() < head + 16 || !data.starts_with(MAGIC) {
        return Err("non è un backup di OtterRoute".into());
    }
    let salt = &data[MAGIC.len()..MAGIC.len() + 16];
    let nonce: [u8; 12] = data[MAGIC.len() + 16..head].try_into().unwrap();
    let key = cipher(&derive(pass, salt)?)?;
    let mut buf = data[head..].to_vec();
    let plain = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(&data[..MAGIC.len() + 16]),
            &mut buf,
        )
        .map_err(|_| "frase segreta errata o file danneggiato".to_string())?;

    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(&plain[..]));
    let mut manifest: Option<Manifest> = None;
    let mut files = Vec::new();
    let mut total = 0usize;
    for entry in ar
        .entries()
        .map_err(|e| format!("archivio non valido: {e}"))?
    {
        let entry = entry.map_err(|e| format!("archivio non valido: {e}"))?;
        if !entry.header().entry_type().is_file() {
            return Err("l'archivio contiene voci non ammesse".into());
        }
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!("percorso non ammesso: {}", path.display()));
        }
        let name = path.to_string_lossy().replace('\\', "/");
        let mut data = Vec::new();
        entry
            .take((MAX_ARCHIVE - total) as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| format!("archivio non valido: {e}"))?;
        total += data.len();
        if total > MAX_ARCHIVE || files.len() > MAX_FILES {
            return Err("archivio troppo grande".into());
        }
        if name == "manifest.json" {
            manifest = Some(
                serde_json::from_slice(&data).map_err(|e| format!("manifest non valido: {e}"))?,
            );
        } else if let Some(rel) = name.strip_prefix("files/").filter(|r| allowed(r)) {
            files.push((rel.to_string(), data));
        } else {
            return Err(format!("voce non ammessa nell'archivio: {name}"));
        }
    }
    let manifest = manifest.ok_or("manca il manifest")?;
    if manifest.format != FORMAT {
        return Err(format!(
            "formato di backup {} non supportato",
            manifest.format
        ));
    }
    if crate::update::is_newer(&manifest.version, crate::update::CURRENT) {
        return Err(format!(
            "il backup è stato creato con la versione {}, più recente di questa ({}): aggiorna prima il nodo",
            manifest.version,
            crate::update::CURRENT
        ));
    }
    if !files.iter().any(|(n, _)| n == "panel.json") {
        return Err("il backup non contiene panel.json".into());
    }
    let mut listed: Vec<&String> = files.iter().map(|(n, _)| n).collect();
    listed.sort();
    let mut declared: Vec<&String> = manifest.files.iter().collect();
    declared.sort();
    if listed != declared {
        return Err("il contenuto non corrisponde al manifest".into());
    }
    Ok(Opened { manifest, files })
}

/// Sostituisce lo stato con quello del backup. Prima salva lo stato attuale in
/// `backups/pre-restore/`. Il nodo va poi riavviato.
pub fn apply(dir: &Path, o: &Opened) -> Result<PathBuf, String> {
    // controlli prima di toccare qualcosa
    let panel = &o
        .files
        .iter()
        .find(|(n, _)| n == "panel.json")
        .ok_or("manca panel.json")?
        .1;
    serde_json::from_slice::<crate::panel::Panel>(panel)
        .map_err(|e| format!("panel.json del backup non valido: {e}"))?;
    if let Some((_, u)) = o.files.iter().find(|(n, _)| n == "users.json") {
        serde_json::from_slice::<serde_json::Value>(u)
            .map_err(|e| format!("users.json del backup non valido: {e}"))?;
    }
    let snap = crate::selfupdate::backup_state(dir, "pre-restore")?;
    for d in ["secrets", "certs"] {
        let _ = std::fs::remove_dir_all(dir.join(d));
    }
    // il pannello per ultimo: se qualcosa si interrompe prima, il vecchio resta coerente
    let mut order: Vec<&(String, Vec<u8>)> = o.files.iter().collect();
    order.sort_by_key(|(n, _)| n == "panel.json");
    for (name, data) in order {
        crate::admin::write_atomic(&dir.join(name), data, true)
            .map_err(|e| format!("scrittura di {name}: {e}"))?;
    }
    Ok(snap)
}

/// Riavvia il processo con lo stesso eseguibile e gli stessi argomenti (stesso PID).
pub fn restart_self() -> std::io::Error {
    use std::os::unix::process::CommandExt;
    match std::env::current_exe() {
        Ok(exe) => std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .exec(),
        Err(e) => e,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        std::fs::write(p.join("panel.json"), b"{}").unwrap();
        std::fs::write(p.join("users.json"), b"{\"users\":[]}").unwrap();
        std::fs::write(p.join("node-id"), b"abc\n").unwrap();
        std::fs::create_dir_all(p.join("secrets")).unwrap();
        std::fs::write(p.join("secrets/b1"), b"segreto").unwrap();
        std::fs::create_dir_all(p.join("certs/a.example.com")).unwrap();
        std::fs::write(p.join("certs/a.example.com/chain.pem"), b"pem").unwrap();
        std::fs::create_dir_all(p.join("cache")).unwrap();
        std::fs::write(p.join("cache/x"), b"no").unwrap();
        d
    }

    #[test]
    fn roundtrip_and_wrong_passphrase() {
        let d = state();
        let data = create(d.path(), "una frase molto lunga", 5).unwrap();
        assert!(!data.windows(7).any(|w| w == b"segreto"), "cifrato");
        let o = open(&data, "una frase molto lunga").unwrap();
        assert_eq!(o.manifest.node_id.as_deref(), Some("abc"));
        assert_eq!(o.files.len(), 5);
        assert!(o.files.iter().all(|(n, _)| !n.starts_with("cache")));
        assert!(open(&data, "un'altra frase lunga")
            .unwrap_err()
            .contains("errata"));
        let mut bad = data.clone();
        *bad.last_mut().unwrap() ^= 1;
        assert!(open(&bad, "una frase molto lunga").is_err());
        assert!(open(b"ciao", "x").is_err());
        assert!(create(d.path(), "corta", 1).is_err());
    }

    #[test]
    fn apply_replaces_state_and_keeps_a_snapshot() {
        let d = state();
        let data = create(d.path(), "una frase molto lunga", 5).unwrap();
        std::fs::write(d.path().join("secrets/nuovo"), b"x").unwrap();
        std::fs::write(d.path().join("secrets/b1"), b"cambiato").unwrap();
        let o = open(&data, "una frase molto lunga").unwrap();
        let snap = apply(d.path(), &o).unwrap();
        assert!(
            snap.join("secrets/nuovo").exists(),
            "stato precedente salvato"
        );
        assert!(!d.path().join("secrets/nuovo").exists());
        assert_eq!(
            std::fs::read(d.path().join("secrets/b1")).unwrap(),
            b"segreto"
        );
        assert!(d.path().join("cache/x").exists(), "la cache non si tocca");
    }

    #[test]
    fn rejects_unsafe_paths_and_newer_versions() {
        assert!(allowed("panel.json") && allowed("secrets/a") && allowed("certs/h/f.pem"));
        assert!(!allowed("../panel.json") && !allowed("secrets/../x") && !allowed("otterroute"));
        assert!(!allowed("secrets/a/b") && !allowed("certs/h") && !allowed("/etc/passwd"));
        let d = state();
        let data = create(d.path(), "una frase molto lunga", 5).unwrap();
        // riscrive il manifest con una versione futura
        let o = open(&data, "una frase molto lunga").unwrap();
        let mut m = o.manifest.clone();
        m.version = "99.0.0".into();
        let forged = forge(&m, &o.files, "una frase molto lunga");
        assert!(open(&forged, "una frase molto lunga")
            .unwrap_err()
            .contains("più recente"));
        m.version = "0.0.1".into();
        assert!(open(
            &forge(&m, &o.files, "una frase molto lunga"),
            "una frase molto lunga"
        )
        .is_ok());
        let mut extra = o.files.clone();
        extra.push(("otterroute".into(), b"x".to_vec()));
        assert!(open(
            &forge(&o.manifest, &extra, "una frase molto lunga"),
            "una frase molto lunga"
        )
        .is_err());
    }

    /// costruisce un archivio a mano (per provare i rifiuti)
    fn forge(m: &Manifest, files: &[(String, Vec<u8>)], pass: &str) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut ar = tar::Builder::new(gz);
        let add = |ar: &mut tar::Builder<_>, name: &str, data: &[u8]| {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o600);
            h.set_cksum();
            ar.append_data(&mut h, name, data).unwrap();
        };
        add(&mut ar, "manifest.json", &serde_json::to_vec(m).unwrap());
        for (n, d) in files {
            add(&mut ar, &format!("files/{n}"), d);
        }
        let mut plain = ar.into_inner().unwrap().finish().unwrap();
        let (salt, nonce) = ([7u8; 16], [9u8; 12]);
        let key = cipher(&derive(pass, &salt).unwrap()).unwrap();
        let mut aad = MAGIC.to_vec();
        aad.extend_from_slice(&salt);
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(&aad),
            &mut plain,
        )
        .unwrap();
        let mut out = aad;
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&plain);
        out
    }
}
