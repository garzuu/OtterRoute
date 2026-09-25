//! Durate leggibili nella configurazione: `90s`, `1h`, `1h30m`, `500ms`, `0`.

use std::time::Duration;

pub fn parse(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s == "0" {
        return Ok(Duration::ZERO);
    }
    if s.is_empty() {
        return Err("durata vuota".into());
    }
    let mut total = Duration::ZERO;
    let mut rest = s;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits == 0 {
            return Err(format!("durata non valida: '{s}'"));
        }
        let n: u64 = rest[..digits]
            .parse()
            .map_err(|_| format!("numero non valido in '{s}'"))?;
        rest = &rest[digits..];
        let unit_len = rest
            .find(|c: char| c.is_ascii_digit())
            .unwrap_or(rest.len());
        let unit = &rest[..unit_len];
        rest = &rest[unit_len..];
        let d = match unit {
            "ms" => Duration::from_millis(n),
            "s" => Duration::from_secs(n),
            "m" => Duration::from_secs(n * 60),
            "h" => Duration::from_secs(n * 3600),
            "d" => Duration::from_secs(n * 86400),
            "" => return Err(format!("unità mancante in '{s}' (ms, s, m, h, d)")),
            u => return Err(format!("unità '{u}' non valida in '{s}' (ms, s, m, h, d)")),
        };
        total += d;
    }
    Ok(total)
}

/// Per `#[serde(with = "crate::duration::serde")]`.
pub mod serde {
    use serde::{Deserialize, Deserializer};
    use std::time::Duration;

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let s = String::deserialize(d)?;
        super::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse("1h30m").unwrap(), Duration::from_secs(5400));
        assert_eq!(parse("500ms").unwrap(), Duration::from_millis(500));
        assert_eq!(parse("0").unwrap(), Duration::ZERO);
        assert_eq!(parse("2d").unwrap(), Duration::from_secs(172800));
        assert!(parse("10").is_err());
        assert!(parse("h").is_err());
        assert!(parse("5y").is_err());
    }
}
