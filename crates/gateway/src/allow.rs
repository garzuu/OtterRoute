//! Indirizzi ammessi al pannello in HTTPS: elenco di IP e reti CIDR.

use std::net::IpAddr;

const MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cidr {
    ip: IpAddr,
    bits: u8,
}

impl Cidr {
    /// `1.2.3.4`, `10.0.0.0/8`, `2001:db8::/32`
    pub fn parse(s: &str) -> Result<Cidr, String> {
        let s = s.trim();
        let (addr, bits) = match s.split_once('/') {
            Some((a, b)) => (a, Some(b)),
            None => (s, None),
        };
        let ip: IpAddr = addr
            .parse()
            .map_err(|_| format!("«{s}» non è un indirizzo IP valido"))?;
        let max = if ip.is_ipv4() { 32 } else { 128 };
        let bits = match bits {
            None => max,
            Some(b) => b
                .parse::<u8>()
                .ok()
                .filter(|b| *b <= max)
                .ok_or_else(|| format!("«{s}»: la lunghezza della rete deve essere 0–{max}"))?,
        };
        Ok(Cidr { ip, bits })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        // un client IPv4 su socket dual-stack arriva come ::ffff:a.b.c.d
        let ip = match ip {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
            v4 => v4,
        };
        match (self.ip, ip) {
            (IpAddr::V4(n), IpAddr::V4(c)) => {
                mask(u32::from(n).into(), u32::from(c).into(), self.bits, 32)
            }
            (IpAddr::V6(n), IpAddr::V6(c)) => mask(u128::from(n), u128::from(c), self.bits, 128),
            _ => false,
        }
    }
}

fn mask(net: u128, ip: u128, bits: u8, width: u32) -> bool {
    if bits == 0 {
        return true;
    }
    let shift = width - u32::from(bits);
    (net >> shift) == (ip >> shift)
}

/// Valida e normalizza l'elenco (senza duplicati, vuoto = nessuna restrizione).
pub fn parse_list(items: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for i in items.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        Cidr::parse(i)?;
        if !out.iter().any(|o| o == i) {
            out.push(i.to_string());
        }
    }
    if out.len() > MAX_ENTRIES {
        return Err(format!("al massimo {MAX_ENTRIES} voci"));
    }
    Ok(out)
}

/// L'elenco è vuoto (tutti ammessi) o contiene l'indirizzo?
pub fn allows(list: &[String], ip: IpAddr) -> bool {
    list.is_empty()
        || list
            .iter()
            .filter_map(|s| Cidr::parse(s).ok())
            .any(|c| c.contains(ip))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn v4_ranges() {
        let l = vec!["10.0.0.0/8".to_string(), "203.0.113.7".to_string()];
        assert!(allows(&l, ip("10.9.8.7")));
        assert!(allows(&l, ip("203.0.113.7")));
        assert!(!allows(&l, ip("203.0.113.8")));
        assert!(!allows(&l, ip("11.0.0.1")));
    }

    #[test]
    fn v6_and_mapped() {
        let l = vec!["2001:db8::/32".to_string(), "192.168.1.0/24".to_string()];
        assert!(allows(&l, ip("2001:db8:1::5")));
        assert!(!allows(&l, ip("2001:db9::1")));
        assert!(allows(&l, ip("::ffff:192.168.1.20")));
    }

    #[test]
    fn empty_allows_all_and_zero_bits() {
        assert!(allows(&[], ip("8.8.8.8")));
        assert!(allows(&["0.0.0.0/0".into()], ip("8.8.8.8")));
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_list(&["10.0.0.0/33".into()]).is_err());
        assert!(parse_list(&["pippo".into()]).is_err());
        assert!(parse_list(&["::1/129".into()]).is_err());
        assert_eq!(
            parse_list(&[" 1.1.1.1 ".into(), "1.1.1.1".into(), "".into()]).unwrap(),
            vec!["1.1.1.1"]
        );
    }
}
