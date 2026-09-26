//! Link firmati con scadenza: `?exp=<unix>&sig=<hex>`.
//!
//! La firma è un HMAC-SHA256, con la chiave del nodo, di `host`, `percorso` e
//! `scadenza`. Chi non conosce la chiave non può fabbricare un link, e cambiare
//! anche solo un carattere del percorso o della scadenza lo invalida.

use std::path::{Path, PathBuf};

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Massima validità che si può dare a un link (365 giorni).
pub const MAX_TTL_SECS: u64 = 365 * 24 * 3600;

fn key_path(state_dir: &Path) -> PathBuf {
    state_dir.join("secrets").join("_signing.json")
}

fn random_key() -> Vec<u8> {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut k = vec![0u8; 32];
    OsRng.fill_bytes(&mut k);
    k
}

fn save(state_dir: &Path, key: &[u8]) -> std::io::Result<()> {
    let p = key_path(state_dir);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    let data = serde_json::json!({ "key": hex::encode(key) }).to_string();
    crate::admin::write_atomic(&p, data.as_bytes(), true)
}

/// La chiave del nodo: si crea al primo avvio e si conserva in un file 0600.
pub fn load_or_create(state_dir: &Path) -> Vec<u8> {
    let existing = std::fs::read(key_path(state_dir))
        .ok()
        .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
        .and_then(|v| v["key"].as_str().and_then(|h| hex::decode(h).ok()))
        .filter(|k| k.len() >= 16);
    if let Some(k) = existing {
        return k;
    }
    let k = random_key();
    if let Err(e) = save(state_dir, &k) {
        tracing::warn!(error = %e, "impossibile salvare la chiave dei link firmati: vale solo fino al riavvio");
    }
    k
}

/// Nuova chiave: tutti i link emessi finora smettono di funzionare.
pub fn rotate(state_dir: &Path) -> std::io::Result<Vec<u8>> {
    let k = random_key();
    save(state_dir, &k)?;
    Ok(k)
}

fn mac(key: &[u8], host: &str, path: &str, exp: u64) -> HmacSha256 {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC accetta chiavi di ogni lunghezza");
    m.update(host.as_bytes());
    m.update(b"\n");
    m.update(path.as_bytes());
    m.update(b"\n");
    m.update(exp.to_string().as_bytes());
    m
}

pub fn sign(key: &[u8], host: &str, path: &str, exp: u64) -> String {
    hex::encode(mac(key, host, path, exp).finalize().into_bytes())
}

/// La parte di query da aggiungere all'indirizzo: `exp=…&sig=…`.
pub fn query(key: &[u8], host: &str, path: &str, exp: u64) -> String {
    format!("exp={exp}&sig={}", sign(key, host, path, exp))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Denied {
    Missing,
    Malformed,
    Expired,
    Invalid,
}

fn param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(name).and_then(|r| r.strip_prefix('=')))
}

/// Controlla `exp` e `sig` nella query. Il chiamante risponde sempre lo stesso
/// `403`, qualunque sia il motivo: chi prova non impara nulla.
pub fn verify(
    key: &[u8],
    host: &str,
    path: &str,
    query: Option<&str>,
    now: u64,
) -> Result<(), Denied> {
    let q = query.unwrap_or_default();
    let (Some(exp), Some(sig)) = (param(q, "exp"), param(q, "sig")) else {
        return Err(Denied::Missing);
    };
    let exp: u64 = exp.parse().map_err(|_| Denied::Malformed)?;
    let sig = hex::decode(sig).map_err(|_| Denied::Malformed)?;
    // prima la firma, poi la scadenza: il confronto è a tempo costante
    mac(key, host, path, exp)
        .verify_slice(&sig)
        .map_err(|_| Denied::Invalid)?;
    if exp <= now {
        return Err(Denied::Expired);
    }
    if exp > now + MAX_TTL_SECS + 3600 {
        return Err(Denied::Invalid);
    }
    Ok(())
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: &[u8] = b"0123456789abcdef0123456789abcdef";

    #[test]
    fn valid_link_passes() {
        let q = query(K, "img.example.com", "/foto/a.jpg", 2000);
        assert_eq!(
            verify(K, "img.example.com", "/foto/a.jpg", Some(&q), 1000),
            Ok(())
        );
        // l'ordine dei parametri e altri parametri non contano
        let swapped = format!(
            "x=1&sig={}&exp=2000",
            sign(K, "img.example.com", "/foto/a.jpg", 2000)
        );
        assert_eq!(
            verify(K, "img.example.com", "/foto/a.jpg", Some(&swapped), 1000),
            Ok(())
        );
    }

    #[test]
    fn expired_missing_and_tampered_are_denied() {
        let q = query(K, "h", "/p", 2000);
        assert_eq!(verify(K, "h", "/p", Some(&q), 2000), Err(Denied::Expired));
        assert_eq!(verify(K, "h", "/p", Some(&q), 3000), Err(Denied::Expired));
        assert_eq!(verify(K, "h", "/p", None, 1000), Err(Denied::Missing));
        assert_eq!(
            verify(K, "h", "/p", Some("exp=2000"), 1000),
            Err(Denied::Missing)
        );
        // percorso, host, scadenza o firma diversi
        assert_eq!(
            verify(K, "h", "/altro", Some(&q), 1000),
            Err(Denied::Invalid)
        );
        assert_eq!(
            verify(K, "altro", "/p", Some(&q), 1000),
            Err(Denied::Invalid)
        );
        let longer = q.replace("exp=2000", "exp=9000");
        assert_eq!(
            verify(K, "h", "/p", Some(&longer), 1000),
            Err(Denied::Invalid)
        );
        let flipped = format!("{}0", &q[..q.len() - 1]);
        assert!(verify(K, "h", "/p", Some(&flipped), 1000).is_err());
        assert_eq!(
            verify(K, "h", "/p", Some("exp=abc&sig=zz"), 1000),
            Err(Denied::Malformed)
        );
        // altra chiave
        assert_eq!(
            verify(
                b"altra-chiave-altra-chiave-altra",
                "h",
                "/p",
                Some(&q),
                1000
            ),
            Err(Denied::Invalid)
        );
    }

    #[test]
    fn far_future_is_refused() {
        let exp = 1000 + MAX_TTL_SECS + 10 * 3600;
        let q = query(K, "h", "/p", exp);
        assert_eq!(verify(K, "h", "/p", Some(&q), 1000), Err(Denied::Invalid));
    }

    #[test]
    fn key_is_persistent_private_and_rotatable() {
        let dir = tempfile::tempdir().unwrap();
        let a = load_or_create(dir.path());
        assert_eq!(a.len(), 32);
        assert_eq!(load_or_create(dir.path()), a, "stessa chiave al riavvio");
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(key_path(dir.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        let b = rotate(dir.path()).unwrap();
        assert_ne!(a, b);
        assert_eq!(load_or_create(dir.path()), b);
        // un link firmato con la chiave vecchia non passa con la nuova
        let q = query(&a, "h", "/p", 5000);
        assert!(verify(&b, "h", "/p", Some(&q), 1000).is_err());
    }
}
