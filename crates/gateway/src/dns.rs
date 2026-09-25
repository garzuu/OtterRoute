//! Verifica dei record DNS di un dominio e rilevamento dell'IP pubblico del nodo.

use std::net::IpAddr;
use std::time::Duration;

use hickory_resolver::config::ResolveHosts;
use hickory_resolver::Resolver;
use serde::{Deserialize, Serialize};

/// Un passaggio del controllo (DNS, destinazione, raggiungibilità del nodo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    pub id: String,
    pub label: String,
    /// "ok", "fail" oppure "skip" (non eseguito perché un passaggio prima è fallito)
    pub status: String,
    pub message: String,
}

fn stage(id: &str, label: &str, status: &str, message: impl Into<String>) -> Stage {
    Stage {
        id: id.into(),
        label: label.into(),
        status: status.into(),
        message: message.into(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub ok: bool,
    /// indirizzi A/AAAA effettivamente trovati
    pub records: Vec<String>,
    /// riepilogo: il primo problema, oppure la conferma
    pub message: String,
    pub stages: Vec<Stage>,
}

/// Percorso con cui un nodo dimostra di essere lui a rispondere su un dominio.
pub const CHECK_PATH: &str = "/.well-known/otterroute/check";

/// Prova che risponde il nodo giusto: hash dell'identità del nodo e del nonce
/// scelto da chi controlla, così una risposta fissa di un altro server non basta.
pub fn proof(node_id: &str, nonce: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(format!("{node_id}:{nonce}").as_bytes()))
}

pub fn is_local_name(host: &str) -> bool {
    host == "localhost" || host.ends_with(".localhost")
}

type DnsResolver = Resolver<hickory_resolver::net::runtime::TokioRuntimeProvider>;

async fn resolve(resolver: &DnsResolver, name: &str) -> Result<Vec<IpAddr>, String> {
    // nome assoluto (punto finale): niente espansione con i domini di ricerca del
    // sistema, che darebbe falsi positivi (es. un DNS jolly del router)
    let fqdn = format!("{name}.");
    match tokio::time::timeout(Duration::from_secs(8), resolver.lookup_ip(fqdn.as_str())).await {
        Err(_) => Err("timeout nell'interrogazione del DNS".into()),
        Ok(Err(_)) => Ok(vec![]),
        Ok(Ok(l)) => Ok(l.iter().collect()),
    }
}

/// Richiesta HTTP reale verso `ip:port` con l'header Host del dominio: il nodo
/// deve rispondere con la prova corretta.
pub async fn reach(host: &str, ip: IpAddr, port: u16, node_id: &str) -> Result<(), String> {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut raw = [0u8; 8];
    OsRng.fill_bytes(&mut raw);
    let nonce = hex::encode(raw);
    let addr = std::net::SocketAddr::new(ip, port);
    let client = crate::s3::build_client(true).map_err(|e| e.to_string())?;
    let resp = client
        .get(format!("http://{addr}{CHECK_PATH}?nonce={nonce}"))
        .header(http::header::HOST, host)
        .timeout(Duration::from_secs(6))
        .send()
        .await
        .map_err(|_| {
            format!(
                "Nessuna risposta su {addr}: controlla che il tuo instradamento (proxy, \
                 load balancer, port forwarding) inoltri la porta {port} a questo nodo."
            )
        })?;
    if !resp.status().is_success() {
        return Err(format!(
            "Su {addr} risponde un altro servizio (stato {}): il dominio non arriva a questo nodo.",
            resp.status().as_u16()
        ));
    }
    let body = resp.text().await.unwrap_or_default();
    if body.contains(&proof(node_id, &nonce)) {
        Ok(())
    } else {
        Err(format!(
            "Su {addr} risponde un altro server, non questo nodo."
        ))
    }
}

fn summary(stages: &[Stage]) -> String {
    stages
        .iter()
        .find(|s| s.status == "fail")
        .or_else(|| stages.last())
        .map(|s| s.message.clone())
        .unwrap_or_default()
}

/// Controlla che il dominio arrivi davvero a questo nodo, in due passaggi:
/// 1. il DNS lo risolve (interrogato direttamente: mai `/etc/hosts`);
/// 2. una richiesta HTTP agli indirizzi trovati, sulla porta di ingresso,
///    riceve la risposta di verifica di questo nodo.
///
/// Non conta dove sia il nodo né come ci si arrivi (proxy, load balancer,
/// tunnel, port forwarding): se la risposta è la sua, il dominio è valido.
/// I nomi `.localhost` non hanno record: i browser li risolvono da soli.
pub async fn check_domain(host: &str, port: u16, node_id: &str) -> CheckResult {
    let done = |ok: bool, records: Vec<String>, stages: Vec<Stage>| CheckResult {
        ok,
        records,
        message: summary(&stages),
        stages,
    };
    if is_local_name(host) {
        return done(
            true,
            vec![],
            vec![stage(
                "local",
                "Dominio locale",
                "ok",
                "Dominio locale (.localhost): il browser lo risolve da solo, nessun record DNS necessario.",
            )],
        );
    }
    let mut stages: Vec<Stage> = Vec::new();
    let label = format!("Il nodo risponde sulla porta {port}");
    let fail_dns = |stages: &mut Vec<Stage>, msg: String| {
        stages.push(stage("dns", "Il dominio risolve", "fail", msg));
        stages.push(stage("reach", &label, "skip", "Non eseguito."));
    };
    let mut builder = match Resolver::builder_tokio() {
        Ok(b) => b,
        Err(e) => {
            fail_dns(&mut stages, format!("resolver DNS non disponibile: {e}"));
            return done(false, vec![], stages);
        }
    };
    builder.options_mut().use_hosts_file = ResolveHosts::Never;
    let resolver = match builder.build() {
        Ok(r) => r,
        Err(e) => {
            fail_dns(&mut stages, format!("resolver DNS non disponibile: {e}"));
            return done(false, vec![], stages);
        }
    };

    // 1. risoluzione
    let ips = match resolve(&resolver, host).await {
        Ok(v) => v,
        Err(e) => {
            fail_dns(&mut stages, e);
            return done(false, vec![], stages);
        }
    };
    let records: Vec<String> = ips.iter().map(|ip| ip.to_string()).collect();
    if ips.is_empty() {
        fail_dns(
            &mut stages,
            format!("Nessun record A/AAAA trovato per {host}."),
        );
        return done(false, records, stages);
    }
    stages.push(stage(
        "dns",
        "Il dominio risolve",
        "ok",
        format!("Risolve a {}.", records.join(", ")),
    ));

    // 2. il nodo risponde attraverso il dominio
    let mut last_err = String::new();
    for ip in &ips {
        match reach(host, *ip, port, node_id).await {
            Ok(()) => {
                stages.push(stage(
                    "reach",
                    &label,
                    "ok",
                    format!("Risposta corretta da {ip}:{port}."),
                ));
                return done(true, records, stages);
            }
            Err(e) => last_err = e,
        }
    }
    stages.push(stage("reach", &label, "fail", last_err));
    done(false, records, stages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn localhost_names_need_no_records() {
        let r = check_domain("img.localhost", 80, "n").await;
        assert!(r.ok);
        assert!(is_local_name("a.b.localhost"));
        assert!(!is_local_name("notlocalhost"));
    }

    #[test]
    fn proof_depends_on_node_and_nonce() {
        assert_eq!(proof("a", "1"), proof("a", "1"));
        assert_ne!(proof("a", "1"), proof("b", "1"));
        assert_ne!(proof("a", "1"), proof("a", "2"));
    }
}
