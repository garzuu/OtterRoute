//! Dalla richiesta pubblica (host + percorso) alla regola e alla chiave dell'oggetto.

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
use std::sync::Arc;

use crate::config::{normalize_host, Route, Snapshot};

#[derive(Debug, PartialEq, Eq)]
pub enum PathError {
    NotAbsolute,
    BadEncoding,
    EncodedSlash,
    DotSegment,
    ControlChar,
}

/// Decodifica e normalizza il percorso della richiesta.
/// - rifiuta `%2F` (ambiguo: separatore o parte del nome?)
/// - rifiuta i segmenti `.` e `..`
/// - comprime le barre multiple
/// - conserva la barra finale (serve a riconoscere le "cartelle")
pub fn normalize_path(raw: &str) -> Result<String, PathError> {
    if !raw.starts_with('/') {
        return Err(PathError::NotAbsolute);
    }
    let lower = raw.to_ascii_lowercase();
    if lower.contains("%2f") || lower.contains("%5c") {
        return Err(PathError::EncodedSlash);
    }
    let decoded = percent_decode_str(raw)
        .decode_utf8()
        .map_err(|_| PathError::BadEncoding)?;
    if decoded.chars().any(|c| c.is_control() || c == '\\') {
        return Err(PathError::ControlChar);
    }
    let trailing = decoded.ends_with('/');
    let mut out = String::with_capacity(decoded.len());
    for seg in decoded.split('/').filter(|s| !s.is_empty()) {
        if seg == "." || seg == ".." {
            return Err(PathError::DotSegment);
        }
        out.push('/');
        out.push_str(seg);
    }
    if out.is_empty() || trailing {
        out.push('/');
    }
    Ok(out)
}

/// Host dalla richiesta: `Host` in HTTP/1.1, authority in HTTP/2.
pub fn request_host(parts: &http::request::Parts) -> Option<String> {
    let raw = parts
        .uri
        .authority()
        .map(|a| a.host().to_owned())
        .or_else(|| {
            let h = parts.headers.get(http::header::HOST)?.to_str().ok()?;
            // togli la porta (attenzione agli IPv6 tra parentesi quadre)
            let host = if h.starts_with('[') {
                h.split(']').next().map(|s| format!("{s}]"))?
            } else {
                h.split(':').next()?.to_owned()
            };
            Some(host)
        })?;
    normalize_host(&raw).ok()
}

impl Snapshot {
    /// Regola con il prefisso più lungo che contiene il percorso.
    /// I prefissi finiscono sempre con `/`, quindi il confronto è per segmenti
    /// interi: `/docs/` non intercetta `/docsx/file`.
    pub fn match_route(&self, host: &str, path: &str) -> Option<&Arc<Route>> {
        self.routes_by_host
            .get(host)?
            .iter()
            .find(|r| path.starts_with(r.path_prefix.as_str()))
    }
}

impl Route {
    /// Chiave dell'oggetto nel bucket. `None` per le "cartelle" (MVP: niente index.html).
    pub fn object_key(&self, path: &str) -> Option<String> {
        let rest = if self.strip_prefix {
            path.strip_prefix(self.path_prefix.as_str())?
        } else {
            path.strip_prefix('/')?
        };
        if rest.is_empty() || rest.ends_with('/') {
            return None;
        }
        Some(format!("{}{}", self.dest.prefix, rest))
    }
}

/// Solo i parametri ammessi dalla politica, ordinati. Tutto il resto viene
/// ignorato e **non** viene mai inoltrato allo storage.
pub fn cache_query(query: Option<&str>, allowed: &[String]) -> String {
    let Some(q) = query else { return String::new() };
    if allowed.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(q.as_bytes())
        .filter(|(k, _)| allowed.iter().any(|a| a == k))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    pairs.sort();
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

/// Chiave di cache. Contiene tutto ciò che identifica l'origine: cambiare
/// storage, bucket o cartella, oppure aumentare `cache_generation`, rende
/// irraggiungibili le copie vecchie senza doverle cancellare.
pub fn cache_key(route: &Route, object_key: &str, query: &str) -> String {
    let d = &route.dest;
    let mut h = Sha256::new();
    for part in [
        route.id.as_str(),
        d.storage.id.as_str(),
        d.bucket.as_str(),
        d.prefix.as_str(),
        &d.revision.to_string(),
        &d.cache_generation.to_string(),
        object_key,
        query,
    ] {
        h.update(part.as_bytes());
        h.update([0u8]);
    }
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    #[test]
    fn normalize() {
        assert_eq!(
            normalize_path("/a//b/./").unwrap_err(),
            PathError::DotSegment
        );
        assert_eq!(normalize_path("/a//b/").unwrap(), "/a/b/");
        assert_eq!(normalize_path("/").unwrap(), "/");
        assert_eq!(
            normalize_path("/foto/barca%20blu.jpg").unwrap(),
            "/foto/barca blu.jpg"
        );
        assert_eq!(
            normalize_path("/a/%2e%2e/b").unwrap_err(),
            PathError::DotSegment
        );
        assert_eq!(
            normalize_path("/a%2Fb").unwrap_err(),
            PathError::EncodedSlash
        );
        assert_eq!(
            normalize_path("/a/%00").unwrap_err(),
            PathError::ControlChar
        );
        assert_eq!(
            normalize_path("/a/%ff").unwrap_err(),
            PathError::BadEncoding
        );
    }

    const Y: &str = r#"
version: 1
storages:
  - { id: a, endpoint: "https://s3.example.com", credentials: { access_key_env: X, secret_key_env: Y } }
destinations:
  - { id: foto, storage: a, bucket: catalogo, prefix: foto/ }
  - { id: docs, storage: a, bucket: documenti, prefix: pubblici }
cache_policies:
  - { id: cp, ttl: 1h, query_keys: [w, v] }
routes:
  - { id: img, host: img.azienda.it, path_prefix: /, destination: foto, cache_policy: cp }
  - { id: mfoto, host: media.azienda.it, path_prefix: /foto/, destination: foto, cache_policy: cp }
  - { id: mdocs, host: media.azienda.it, path_prefix: /docs/, destination: docs, cache_policy: cp }
  - { id: mraw, host: media.azienda.it, path_prefix: /raw/, strip_prefix: false, destination: docs, cache_policy: cp }
"#;

    fn snap() -> Snapshot {
        config::validate(serde_yaml::from_str(Y).unwrap(), &|k| Some(k.to_string())).unwrap()
    }

    fn resolve(s: &Snapshot, host: &str, path: &str) -> Option<(String, String)> {
        let r = s.match_route(host, path)?;
        Some((r.dest.bucket.clone(), r.object_key(path)?))
    }

    #[test]
    fn examples_from_the_analysis() {
        let s = snap();
        assert_eq!(
            resolve(&s, "img.azienda.it", "/barca.jpg"),
            Some(("catalogo".into(), "foto/barca.jpg".into()))
        );
        assert_eq!(
            resolve(&s, "media.azienda.it", "/foto/barca.jpg"),
            Some(("catalogo".into(), "foto/barca.jpg".into()))
        );
        assert_eq!(
            resolve(&s, "media.azienda.it", "/docs/listino.pdf"),
            Some(("documenti".into(), "pubblici/listino.pdf".into()))
        );
        assert_eq!(
            resolve(&s, "media.azienda.it", "/raw/x.pdf"),
            Some(("documenti".into(), "pubblici/raw/x.pdf".into()))
        );
    }

    #[test]
    fn segment_boundaries_and_directories() {
        let s = snap();
        assert!(s.match_route("media.azienda.it", "/docsx/a.pdf").is_none());
        assert!(resolve(&s, "media.azienda.it", "/docs/").is_none());
        assert!(resolve(&s, "img.azienda.it", "/cartella/").is_none());
        assert!(s.match_route("altro.it", "/a").is_none());
    }

    #[test]
    fn query_filter_and_cache_key() {
        assert_eq!(
            cache_query(Some("z=1&w=200&v=3&w=100"), &["v".into(), "w".into()]),
            "v=3&w=100&w=200"
        );
        assert_eq!(cache_query(Some("acl&x=1"), &[]), "");
        let s = snap();
        let img = s.match_route("img.azienda.it", "/a.jpg").unwrap();
        let mf = s.match_route("media.azienda.it", "/foto/a.jpg").unwrap();
        // stessa destinazione, regole diverse: cache separate
        assert_ne!(
            cache_key(img, "foto/a.jpg", ""),
            cache_key(mf, "foto/a.jpg", "")
        );
        assert_ne!(
            cache_key(img, "foto/a.jpg", ""),
            cache_key(img, "foto/a.jpg", "w=1")
        );
    }
}
