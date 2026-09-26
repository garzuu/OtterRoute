//! Diagnosi "perché non funziona": il percorso di una richiesta (DNS → nodo →
//! instradamento → cache → storage → risposta) con il passaggio che fallisce e
//! cosa fare. Qui stanno i pezzi puri (passi, rimedi, report); la raccolta dei dati
//! sta in `admin::diagnose`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub id: &'static str,
    pub label: String,
    pub status: Status,
    pub detail: String,
    /// cosa fare, solo per warn e fail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Step {
    pub fn new(
        id: &'static str,
        label: impl Into<String>,
        status: Status,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id,
            label: label.into(),
            status,
            detail: detail.into(),
            fix: None,
        }
    }
    pub fn fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }
    pub fn skipped(id: &'static str, label: impl Into<String>) -> Self {
        Self::new(
            id,
            label,
            Status::Skip,
            "Non eseguito: un passaggio precedente è fallito.",
        )
    }
}

/// Il primo passaggio che fallisce, cioè la causa più probabile.
pub fn first_failure(steps: &[Step]) -> Option<&Step> {
    steps.iter().find(|s| s.status == Status::Fail)
}

pub fn summary(steps: &[Step]) -> String {
    match first_failure(steps) {
        Some(s) => format!("Si ferma a «{}»: {}", s.label, s.detail),
        None if steps.iter().any(|s| s.status == Status::Warn) => {
            "Funziona, con qualche avvertenza.".into()
        }
        None => {
            "Tutto in ordine: la richiesta arriva, viene instradata e il file è leggibile.".into()
        }
    }
}

/// Testo da incollare in una segnalazione: nessun segreto, solo esiti.
pub fn report(url: &str, node: &str, when: &str, steps: &[Step]) -> String {
    let mut out = format!("Diagnosi OtterRoute\nURL: {url}\nNodo: {node}\nData: {when}\n\n");
    for s in steps {
        let mark = match s.status {
            Status::Ok => "[ok]   ",
            Status::Warn => "[warn] ",
            Status::Fail => "[FAIL] ",
            Status::Skip => "[skip] ",
        };
        out.push_str(&format!("{mark}{} — {}\n", s.label, s.detail));
        if let Some(f) = &s.fix {
            out.push_str(&format!("       → {f}\n"));
        }
    }
    out.push_str(&format!("\n{}\n", summary(steps)));
    out
}

/// Descrizione dello stato di una copia in cache.
pub fn cache_state(
    status: u16,
    age_secs: u64,
    fresh: bool,
    stale_usable: bool,
    ttl_secs: u64,
) -> (Status, String) {
    if status == 404 {
        let d = format!("In cache c'è un «non trovato» (cache negativa, {age_secs} s fa): il file risulta assente.");
        return (Status::Warn, d);
    }
    if fresh {
        (Status::Ok, format!("Copia fresca in cache ({age_secs} s su {ttl_secs} s): la risposta arriva subito (HIT)."))
    } else if stale_usable {
        (Status::Warn, format!("Copia scaduta ({age_secs} s): al prossimo accesso si rivalida con lo storage; se non risponde viene servita comunque (STALE)."))
    } else {
        (
            Status::Warn,
            format!("Copia troppo vecchia ({age_secs} s): verrà riletta dallo storage."),
        )
    }
}

/// Rimedio per un esito della verifica dello storage.
pub fn storage_fix(outcome: &str) -> &'static str {
    match outcome {
        "not_found" => "Controlla nome del file, cartella dell'instradamento e permessi di lettura della chiave (s3:GetObject). Un file mancante dà lo stesso errore di un permesso mancante.",
        "auth" => "Ricontrolla chiavi, regione e stile d'indirizzamento (path o virtual host) nel bucket; verifica l'orologio della macchina (SigV4 tollera 15 minuti).",
        "unreachable" => "Controlla l'endpoint, il DNS dello storage, il firewall e, se lo storage è in rete locale, la casella «rete locale».",
        _ => "Controlla la configurazione del bucket.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_points_at_the_first_failure() {
        let steps = vec![
            Step::new("dns", "Il dominio risolve", Status::Ok, "1.2.3.4"),
            Step::new("route", "Instradamento", Status::Fail, "nessuna regola")
                .fix("crea un instradamento"),
            Step::skipped("cache", "Cache"),
        ];
        assert_eq!(first_failure(&steps).unwrap().id, "route");
        assert!(summary(&steps).contains("Instradamento"));
        let r = report("http://a.it/x", "nodo 1234", "2026-01-01", &steps);
        assert!(
            r.contains("[FAIL] Instradamento — nessuna regola")
                && r.contains("→ crea un instradamento")
        );
        assert!(r.contains("[skip] Cache") && r.contains("[ok]   Il dominio risolve"));
    }

    #[test]
    fn all_ok_and_warnings() {
        let ok = vec![Step::new("a", "A", Status::Ok, "")];
        assert!(summary(&ok).starts_with("Tutto in ordine"));
        let w = vec![Step::new("a", "A", Status::Warn, "")];
        assert!(summary(&w).contains("avvertenza"));
    }

    #[test]
    fn cache_states() {
        assert_eq!(cache_state(200, 10, true, true, 3600).0, Status::Ok);
        assert_eq!(cache_state(200, 4000, false, true, 3600).0, Status::Warn);
        assert!(cache_state(404, 5, true, false, 60)
            .1
            .contains("cache negativa"));
        assert!(cache_state(200, 999_999, false, false, 3600)
            .1
            .contains("riletta"));
    }
}
