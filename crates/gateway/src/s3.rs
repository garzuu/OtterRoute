//! Client S3 minimale: firma SigV4, GET/HEAD di un oggetto e classificazione
//! degli errori. Scritto a mano di proposito: servono solo due operazioni e
//! vogliamo il controllo completo su header, timeout e risoluzione DNS.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use http::Method;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use sha2::{Digest, Sha256};

use crate::config::{Addressing, Storage};

pub const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

// ---------------------------------------------------------------------------
// Client HTTP e protezione dagli endpoint interni
// ---------------------------------------------------------------------------

pub fn build_client(allow_private: bool) -> reqwest::Result<reqwest::Client> {
    let mut b = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("otterroute/", env!("CARGO_PKG_VERSION")));
    if !allow_private {
        // Il controllo sul nome in configurazione non basta: un nome pubblico
        // può risolvere verso 127.0.0.1. Filtriamo gli indirizzi risolti.
        b = b.dns_resolver(std::sync::Arc::new(PublicOnlyResolver));
    }
    b.build()
}

struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| !is_private_ip(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err(format!("{host}: nessun indirizzo pubblico").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10
                || o[0] == 0
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_ip(IpAddr::V4(v4));
            }
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10
        }
    }
}

// ---------------------------------------------------------------------------
// Firma SigV4
// ---------------------------------------------------------------------------

/// Codifica S3: tutto tranne `A-Z a-z 0-9 - _ . ~` (e `/` se richiesto).
pub fn uri_encode(s: &str, keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len() * 3 / 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b'/' if keep_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accetta chiavi di ogni lunghezza");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

pub struct SignInput<'a> {
    pub method: &'a str,
    pub host: &'a str,
    pub canonical_uri: &'a str,
    /// header aggiuntivi da firmare (nomi in minuscolo), es. range, if-none-match
    pub extra_headers: &'a [(String, String)],
    pub payload_hash: &'a str,
    pub access_key: &'a str,
    pub secret_key: &'a str,
    pub region: &'a str,
    pub now: DateTime<Utc>,
}

/// Restituisce gli header da aggiungere alla richiesta (x-amz-date,
/// x-amz-content-sha256, authorization).
pub fn sign(i: &SignInput) -> Vec<(String, String)> {
    let amz_date = i.now.format("%Y%m%dT%H%M%SZ").to_string();
    let date = i.now.format("%Y%m%d").to_string();

    let mut headers: Vec<(String, String)> = vec![
        ("host".into(), i.host.to_owned()),
        ("x-amz-content-sha256".into(), i.payload_hash.to_owned()),
        ("x-amz-date".into(), amz_date.clone()),
    ];
    for (k, v) in i.extra_headers {
        headers.push((k.to_ascii_lowercase(), v.trim().to_owned()));
    }
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed_headers = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        i.method, i.canonical_uri, canonical_headers, signed_headers, i.payload_hash
    );
    let scope = format!("{date}/{}/s3/aws4_request", i.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );

    let k_date = hmac(format!("AWS4{}", i.secret_key).as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, i.region.as_bytes());
    let k_service = hmac(&k_region, b"s3");
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

    vec![
        ("x-amz-date".into(), amz_date),
        ("x-amz-content-sha256".into(), i.payload_hash.to_owned()),
        (
            "authorization".into(),
            format!(
                "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
                i.access_key
            ),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Richiesta di un oggetto
// ---------------------------------------------------------------------------

/// URL, header Host e URI canonico per un oggetto.
pub fn object_location(st: &Storage, bucket: &str, key: &str) -> (url::Url, String, String) {
    let enc_key = uri_encode(key, true);
    let mut url = st.endpoint.clone();
    let canonical_uri = match st.addressing {
        Addressing::Path => format!("/{}/{}", uri_encode(bucket, false), enc_key),
        Addressing::Virtual => {
            let h = format!("{bucket}.{}", st.endpoint.host_str().unwrap_or_default());
            let _ = url.set_host(Some(&h));
            format!("/{enc_key}")
        }
    };
    url.set_path(&canonical_uri);
    let host = match url.port() {
        Some(p) => format!("{}:{p}", url.host_str().unwrap_or_default()),
        None => url.host_str().unwrap_or_default().to_owned(),
    };
    (url, host, canonical_uri)
}

#[derive(Debug)]
pub enum Fetch {
    /// 200, 206 o 416: da inoltrare
    Ok(reqwest::Response),
    /// 304 su richiesta condizionale del gateway
    NotModified,
    /// oggetto inesistente (404, NoSuchKey, oppure 403 AccessDenied senza ListBucket)
    NotFound,
    /// credenziali o firma sbagliate: errore di configurazione, mai in cache
    Misconfigured(String),
    /// storage irraggiungibile, timeout, 5xx
    Upstream(String),
}

pub async fn fetch(
    st: &Storage,
    bucket: &str,
    key: &str,
    method: &Method,
    extra_headers: &[(String, String)],
) -> Fetch {
    let resp = match send(st, bucket, key, method, extra_headers).await {
        Ok(r) => r,
        Err(e) => return Fetch::Upstream(e.to_string()),
    };
    let status = resp.status();
    match status.as_u16() {
        200 | 206 | 416 => Fetch::Ok(resp),
        304 => Fetch::NotModified,
        404 => Fetch::NotFound,
        403 => {
            if method == Method::HEAD {
                // HEAD non ha corpo: non sappiamo se è "file mancante" o
                // "credenziali sbagliate". Chiediamo un byte con GET per capirlo.
                let probe = [("range".to_string(), "bytes=0-0".to_string())];
                return match send(st, bucket, key, &Method::GET, &probe).await {
                    Ok(r) if r.status().as_u16() == 403 => classify_403(r).await,
                    Ok(r) if r.status().as_u16() == 404 => Fetch::NotFound,
                    Ok(r) if r.status().is_success() => {
                        Fetch::Upstream("HEAD 403 ma GET riuscito".into())
                    }
                    Ok(r) => Fetch::Upstream(format!("stato inatteso {}", r.status())),
                    Err(e) => Fetch::Upstream(e.to_string()),
                };
            }
            classify_403(resp).await
        }
        s if s >= 500 => Fetch::Upstream(format!("storage ha risposto {status}")),
        _ => {
            let body = resp.text().await.unwrap_or_default();
            Fetch::Misconfigured(format!(
                "stato {status}: {}",
                s3_error_code(&body).unwrap_or("?")
            ))
        }
    }
}

async fn classify_403(resp: reqwest::Response) -> Fetch {
    let body = resp.text().await.unwrap_or_default();
    match s3_error_code(&body) {
        Some("AccessDenied") | Some("NoSuchKey") => Fetch::NotFound,
        Some(code) => Fetch::Misconfigured(format!("403 {code}")),
        None => Fetch::Misconfigured("403 senza codice".into()),
    }
}

fn s3_error_code(body: &str) -> Option<&str> {
    let start = body.find("<Code>")? + "<Code>".len();
    let end = body[start..].find("</Code>")? + start;
    Some(&body[start..end])
}

async fn send(
    st: &Storage,
    bucket: &str,
    key: &str,
    method: &Method,
    extra_headers: &[(String, String)],
) -> reqwest::Result<reqwest::Response> {
    let (url, host, canonical_uri) = object_location(st, bucket, key);
    let signed = sign(&SignInput {
        method: method.as_str(),
        host: &host,
        canonical_uri: &canonical_uri,
        extra_headers,
        payload_hash: UNSIGNED_PAYLOAD,
        access_key: &st.access_key,
        secret_key: &st.secret_key,
        region: &st.region,
        now: Utc::now(),
    });
    let mut req = st.client.request(method.clone(), url);
    for (k, v) in extra_headers.iter().chain(signed.iter()) {
        req = req.header(k.as_str(), v.as_str());
    }
    req.send().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// Esempio ufficiale AWS "GET Object" (documentazione SigV4 di S3).
    #[test]
    fn aws_get_object_vector() {
        let extra = vec![("range".to_string(), "bytes=0-9".to_string())];
        let h = sign(&SignInput {
            method: "GET",
            host: "examplebucket.s3.amazonaws.com",
            canonical_uri: "/test.txt",
            extra_headers: &extra,
            payload_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            access_key: "AKIAIOSFODNN7EXAMPLE",
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            region: "us-east-1",
            now: Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap(),
        });
        let auth = &h.iter().find(|(k, _)| k == "authorization").unwrap().1;
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
    }

    #[test]
    fn encoding() {
        assert_eq!(
            uri_encode("foto/barca blu+1.jpg", true),
            "foto/barca%20blu%2B1.jpg"
        );
        assert_eq!(uri_encode("à", true), "%C3%A0");
        assert_eq!(uri_encode("a/b", false), "a%2Fb");
    }

    #[test]
    fn private_ips() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(is_private_ip(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "2a01:4f8::1"] {
            assert!(!is_private_ip(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn error_code() {
        assert_eq!(
            s3_error_code("<Error><Code>NoSuchKey</Code></Error>"),
            Some("NoSuchKey")
        );
        assert_eq!(s3_error_code("boh"), None);
    }
}
