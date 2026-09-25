//! Telegram Bot API: `sendMessage` a una o più chat.

use super::TelegramCfg;

pub const API_BASE: &str = "https://api.telegram.org";

/// Manda `text` a tutte le chat configurate. Il token non compare mai negli errori.
pub async fn send(base: &str, cfg: &TelegramCfg, token: &str, text: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("manca il token del bot".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{base}/bot{token}/sendMessage");
    let mut errors = Vec::new();
    for chat in &cfg.chat_ids {
        let res = client
            .post(&url)
            .header("content-type", "application/json")
            .body(serde_json::json!({ "chat_id": chat, "text": text, "disable_web_page_preview": true }).to_string())
            .send()
            .await;
        match res {
            Err(e) => errors.push(format!("{chat}: {}", e.without_url())),
            Ok(r) if r.status().is_success() => {}
            Ok(r) => {
                let status = r.status();
                let desc = r
                    .text()
                    .await
                    .ok()
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .and_then(|v| v["description"].as_str().map(str::to_string))
                    .unwrap_or_else(|| status.to_string());
                errors.push(format!("{chat}: {desc}"));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
