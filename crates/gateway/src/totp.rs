//! Secondo fattore: TOTP (RFC 6238) compatibile con Google Authenticator,
//! 1Password, Authy e simili (SHA-1, 6 cifre, passo di 30 secondi).

use argon2::password_hash::rand_core::{OsRng, RngCore};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const STEP_SECS: u64 = 30;
/// Tolleranza sull'orologio: un passo prima e uno dopo.
const WINDOW: i64 = 1;

pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in data {
        buf = (buf << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            out.push(B32[((buf >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(B32[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let (mut buf, mut bits) = (0u32, 0u32);
    let mut out = Vec::new();
    for c in s.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let v = B32
            .iter()
            .position(|&b| b as char == c.to_ascii_uppercase())? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push((buf >> (bits - 8)) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// Nuovo segreto casuale (20 byte, come raccomanda la RFC), in base32.
pub fn generate_secret() -> String {
    let mut raw = [0u8; 20];
    OsRng.fill_bytes(&mut raw);
    base32_encode(&raw)
}

fn hotp(key: &[u8], counter: u64) -> u32 {
    let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC accetta chiavi di ogni lunghezza");
    mac.update(&counter.to_be_bytes());
    let h = mac.finalize().into_bytes();
    let off = (h[19] & 0x0f) as usize;
    let bin = (u32::from(h[off] & 0x7f) << 24)
        | (u32::from(h[off + 1]) << 16)
        | (u32::from(h[off + 2]) << 8)
        | u32::from(h[off + 3]);
    bin % 1_000_000
}

/// Codice a 6 cifre per il passo `step` (solo test: le app lo calcolano da sole).
#[cfg(test)]
pub fn code_at(secret_b32: &str, step: u64) -> Option<String> {
    let key = base32_decode(secret_b32)?;
    Some(format!("{:06}", hotp(&key, step)))
}

pub fn step_of(now_secs: u64) -> u64 {
    now_secs / STEP_SECS
}

/// Verifica un codice. Restituisce il passo accettato, che va salvato come
/// "ultimo usato": un passo già consumato non vale più (niente riuso del codice).
pub fn verify(secret_b32: &str, code: &str, now_secs: u64, last_step: u64) -> Option<u64> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let key = base32_decode(secret_b32)?;
    let cur = step_of(now_secs) as i64;
    for d in -WINDOW..=WINDOW {
        let step = (cur + d).max(0) as u64;
        if step > last_step && format!("{:06}", hotp(&key, step)) == code {
            return Some(step);
        }
    }
    None
}

pub fn otpauth_url(issuer: &str, account: &str, secret_b32: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect::<String>()
    };
    format!(
        "otpauth://totp/{}:{}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits=6&period=30",
        enc(issuer),
        enc(account),
        enc(issuer)
    )
}

/// QR code come SVG, da inserire direttamente nella pagina.
pub fn qr_svg(data: &str) -> Option<String> {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    Some(
        code.render::<svg::Color>()
            .min_dimensions(200, 200)
            .quiet_zone(true)
            .dark_color(svg::Color("#0f2537"))
            .light_color(svg::Color("#ffffff"))
            .build(),
    )
}

// --- codici di recupero -----------------------------------------------------

const RECOVERY_ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";

/// 10 codici monouso, nel formato `xxxxx-xxxxx`.
pub fn generate_recovery_codes() -> Vec<String> {
    (0..10)
        .map(|_| {
            let mut raw = [0u8; 10];
            OsRng.fill_bytes(&mut raw);
            let s: String = raw
                .iter()
                .map(|b| RECOVERY_ALPHABET[*b as usize % RECOVERY_ALPHABET.len()] as char)
                .collect();
            format!("{}-{}", &s[..5], &s[5..])
        })
        .collect()
}

/// Hash per salvarli: sono già ad alta entropia, basta SHA-256.
pub fn hash_recovery_code(code: &str) -> String {
    let norm: String = code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    hex::encode(Sha256::digest(norm.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vettori di prova della RFC 6238 (SHA-1, segreto ASCII "12345678901234567890"),
    /// ridotti alle ultime 6 cifre.
    #[test]
    fn rfc6238_vectors() {
        let secret = base32_encode(b"12345678901234567890");
        for (t, code) in [
            (59u64, "287082"),
            (1_111_111_109, "081804"),
            (1_111_111_111, "050471"),
            (1_234_567_890, "005924"),
            (2_000_000_000, "279037"),
        ] {
            assert_eq!(code_at(&secret, step_of(t)).unwrap(), code, "t={t}");
        }
    }

    #[test]
    fn window_and_replay() {
        let secret = generate_secret();
        let now = 1_800_000_000;
        let cur = step_of(now);
        let code = code_at(&secret, cur).unwrap();
        // il passo corrente e quelli adiacenti valgono
        assert_eq!(verify(&secret, &code, now, 0), Some(cur));
        assert_eq!(verify(&secret, &code, now + 30, 0), Some(cur));
        assert_eq!(verify(&secret, &code, now - 30, 0), Some(cur));
        // troppo lontano nel tempo: no
        assert_eq!(verify(&secret, &code, now + 120, 0), None);
        // già usato: no
        assert_eq!(verify(&secret, &code, now, cur), None);
        // formato sbagliato
        assert_eq!(verify(&secret, "12345", now, 0), None);
        assert_eq!(verify(&secret, "abcdef", now, 0), None);
        // spazi tollerati (le app spesso mostrano "123 456")
        let spaced = format!("{} {}", &code[..3], &code[3..]);
        assert_eq!(verify(&secret, &spaced, now, 0), Some(cur));
    }

    #[test]
    fn base32_roundtrip() {
        for data in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foobar",
            &[0u8, 255, 17, 42, 99],
        ] {
            assert_eq!(base32_decode(&base32_encode(data)).unwrap(), data);
        }
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert!(base32_decode("!!").is_none());
    }

    #[test]
    fn otpauth_and_qr() {
        let url = otpauth_url("OtterRoute", "a b@x", "ABC234");
        assert!(
            url.starts_with("otpauth://totp/OtterRoute:a%20b%40x?secret=ABC234&issuer=OtterRoute")
        );
        assert!(qr_svg(&url).unwrap().contains("<svg"));
    }

    #[test]
    fn recovery_codes() {
        let codes = generate_recovery_codes();
        assert_eq!(codes.len(), 10);
        assert!(codes
            .iter()
            .all(|c| c.len() == 11 && c.as_bytes()[5] == b'-'));
        // stesso hash con o senza trattino e maiuscole
        let c = &codes[0];
        assert_eq!(
            hash_recovery_code(c),
            hash_recovery_code(&c.replace('-', "").to_uppercase())
        );
        assert_ne!(hash_recovery_code(&codes[0]), hash_recovery_code(&codes[1]));
    }
}
