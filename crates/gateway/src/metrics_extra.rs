//! Metriche di funzioni più recenti: immagini al volo, link firmati, certificati e aggiornamenti.
//! I contatori sono globali al processo; i valori "di stato" (scadenza dei certificati, versione)
//! si leggono al momento della richiesta a `/metrics`.

use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use crate::tls::CertInfo;

#[derive(Default)]
pub struct Counters {
    pub img_ok: AtomicU64,
    pub img_error: AtomicU64,
    pub img_too_large: AtomicU64,
    pub img_busy: AtomicU64,
    /// durata totale delle trasformazioni riuscite, in microsecondi
    pub img_micros: AtomicU64,
    pub signed_denied: AtomicU64,
    pub acme_ok: AtomicU64,
    pub acme_error: AtomicU64,
    pub update_check_ok: AtomicU64,
    pub update_check_error: AtomicU64,
    pub update_applied: AtomicU64,
    pub update_rollback: AtomicU64,
}

pub static C: Counters = Counters {
    img_ok: AtomicU64::new(0),
    img_error: AtomicU64::new(0),
    img_too_large: AtomicU64::new(0),
    img_busy: AtomicU64::new(0),
    img_micros: AtomicU64::new(0),
    signed_denied: AtomicU64::new(0),
    acme_ok: AtomicU64::new(0),
    acme_error: AtomicU64::new(0),
    update_check_ok: AtomicU64::new(0),
    update_check_error: AtomicU64::new(0),
    update_applied: AtomicU64::new(0),
    update_rollback: AtomicU64::new(0),
};

pub fn inc(c: &AtomicU64) {
    c.fetch_add(1, Relaxed);
}

/// Stato letto al momento della richiesta.
pub struct Snapshot<'a> {
    pub version: &'a str,
    pub install: &'a str,
    pub certs: &'a [(String, CertInfo, bool)],
    pub update_available: bool,
    pub update_checked_at: u64,
    pub now: u64,
}

fn esc(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

pub fn render(s: &Snapshot<'_>) -> String {
    let mut o = String::new();
    fn metric_head(o: &mut String, name: &str, kind: &str, help: &str) {
        let _ = writeln!(o, "# HELP {name} {help}\n# TYPE {name} {kind}");
    }
    metric_head(
        &mut o,
        "otterroute_build_info",
        "gauge",
        "Versione in esecuzione e tipo di installazione.",
    );
    let _ = writeln!(
        o,
        "otterroute_build_info{{version=\"{}\",install=\"{}\"}} 1",
        esc(s.version),
        esc(s.install)
    );

    metric_head(
        &mut o,
        "otterroute_image_transforms_total",
        "counter",
        "Trasformazioni di immagini al volo, per esito.",
    );
    for (r, c) in [
        ("ok", &C.img_ok),
        ("error", &C.img_error),
        ("too_large", &C.img_too_large),
        ("busy", &C.img_busy),
    ] {
        let _ = writeln!(
            o,
            "otterroute_image_transforms_total{{result=\"{r}\"}} {}",
            c.load(Relaxed)
        );
    }
    metric_head(
        &mut o,
        "otterroute_image_transform_seconds_sum",
        "counter",
        "Tempo totale delle trasformazioni riuscite.",
    );
    let _ = writeln!(
        o,
        "otterroute_image_transform_seconds_sum {:.6}",
        C.img_micros.load(Relaxed) as f64 / 1e6
    );
    metric_head(
        &mut o,
        "otterroute_image_transform_seconds_count",
        "counter",
        "Numero di trasformazioni riuscite misurate.",
    );
    let _ = writeln!(
        o,
        "otterroute_image_transform_seconds_count {}",
        C.img_ok.load(Relaxed)
    );

    metric_head(
        &mut o,
        "otterroute_signed_links_denied_total",
        "counter",
        "Richieste rifiutate (403) per link firmato assente, scaduto o non valido.",
    );
    let _ = writeln!(
        o,
        "otterroute_signed_links_denied_total {}",
        C.signed_denied.load(Relaxed)
    );

    metric_head(
        &mut o,
        "otterroute_acme_issuances_total",
        "counter",
        "Emissioni di certificati ACME, per esito.",
    );
    let _ = writeln!(
        o,
        "otterroute_acme_issuances_total{{result=\"ok\"}} {}",
        C.acme_ok.load(Relaxed)
    );
    let _ = writeln!(
        o,
        "otterroute_acme_issuances_total{{result=\"error\"}} {}",
        C.acme_error.load(Relaxed)
    );

    metric_head(
        &mut o,
        "otterroute_certificate_not_after_timestamp_seconds",
        "gauge",
        "Scadenza del certificato del dominio (secondi Unix).",
    );
    for (host, info, _) in s.certs {
        if let Some(na) = info.not_after {
            let _ = writeln!(
                o,
                "otterroute_certificate_not_after_timestamp_seconds{{host=\"{}\"}} {na}",
                esc(host)
            );
        }
    }
    metric_head(
        &mut o,
        "otterroute_certificate_serving",
        "gauge",
        "1 se il nodo serve un certificato valido per il dominio su HTTPS.",
    );
    for (host, info, serving) in s.certs {
        let ok = *serving && info.not_after.is_some_and(|na| na > s.now);
        let _ = writeln!(
            o,
            "otterroute_certificate_serving{{host=\"{}\"}} {}",
            esc(host),
            u8::from(ok)
        );
    }

    metric_head(
        &mut o,
        "otterroute_update_available",
        "gauge",
        "1 se è disponibile una versione più recente di quella in uso.",
    );
    let _ = writeln!(
        o,
        "otterroute_update_available {}",
        u8::from(s.update_available)
    );
    metric_head(
        &mut o,
        "otterroute_update_last_check_timestamp_seconds",
        "gauge",
        "Ultimo controllo delle nuove versioni (0 = mai).",
    );
    let _ = writeln!(
        o,
        "otterroute_update_last_check_timestamp_seconds {}",
        s.update_checked_at
    );
    metric_head(
        &mut o,
        "otterroute_update_checks_total",
        "counter",
        "Controlli delle nuove versioni, per esito.",
    );
    let _ = writeln!(
        o,
        "otterroute_update_checks_total{{result=\"ok\"}} {}",
        C.update_check_ok.load(Relaxed)
    );
    let _ = writeln!(
        o,
        "otterroute_update_checks_total{{result=\"error\"}} {}",
        C.update_check_error.load(Relaxed)
    );
    metric_head(
        &mut o,
        "otterroute_updates_total",
        "counter",
        "Aggiornamenti installati e annullati (rollback) da questo processo.",
    );
    let _ = writeln!(
        o,
        "otterroute_updates_total{{result=\"applied\"}} {}",
        C.update_applied.load(Relaxed)
    );
    let _ = writeln!(
        o,
        "otterroute_updates_total{{result=\"rolled_back\"}} {}",
        C.update_rollback.load(Relaxed)
    );
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_counters_and_state() {
        inc(&C.img_ok);
        C.img_micros.fetch_add(1_500_000, Relaxed);
        inc(&C.signed_denied);
        let info = CertInfo {
            host: "a.example.com".into(),
            not_after: Some(2_000_000_000),
            issued_at: Some(1),
            error: None,
        };
        let certs = vec![
            ("a.example.com".to_string(), info.clone(), true),
            (
                "b.example.com".to_string(),
                CertInfo {
                    host: "b.example.com".into(),
                    not_after: None,
                    issued_at: None,
                    error: None,
                },
                false,
            ),
        ];
        let out = render(&Snapshot {
            version: "0.1.1",
            install: "docker",
            certs: &certs,
            update_available: true,
            update_checked_at: 42,
            now: 1_000_000_000,
        });
        assert!(out.contains("otterroute_build_info{version=\"0.1.1\",install=\"docker\"} 1"));
        assert!(out.contains("otterroute_image_transforms_total{result=\"ok\"}"));
        assert!(out.contains("otterroute_image_transform_seconds_sum"));
        assert!(out.contains("otterroute_signed_links_denied_total "));
        assert!(out.contains(
            "otterroute_certificate_not_after_timestamp_seconds{host=\"a.example.com\"} 2000000000"
        ));
        assert!(
            !out.contains("not_after_timestamp_seconds{host=\"b.example.com\"}"),
            "senza certificato niente scadenza"
        );
        assert!(out.contains("otterroute_certificate_serving{host=\"a.example.com\"} 1"));
        assert!(out.contains("otterroute_certificate_serving{host=\"b.example.com\"} 0"));
        // un certificato scaduto non è "in uso"
        let expired = vec![(
            "c.it".to_string(),
            CertInfo {
                host: "c.it".into(),
                not_after: Some(5),
                issued_at: Some(1),
                error: None,
            },
            true,
        )];
        let out2 = render(&Snapshot {
            version: "x",
            install: "y",
            certs: &expired,
            update_available: false,
            update_checked_at: 0,
            now: 100,
        });
        assert!(out2.contains("otterroute_certificate_serving{host=\"c.it\"} 0"));
        assert!(out2.contains("otterroute_update_available 0"));
        // ogni serie ha HELP e TYPE
        assert_eq!(out.matches("# HELP").count(), out.matches("# TYPE").count());
        // le etichette si proteggono
        assert_eq!(esc("a\"b\\c\nd"), "a\\\"b\\\\c d");
    }
}
