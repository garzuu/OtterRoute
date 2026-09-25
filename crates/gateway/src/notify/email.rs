//! Email via SMTP (lettre).

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use super::{EmailCfg, Security};

pub fn parse_mailbox(s: &str) -> Result<Mailbox, String> {
    if s.chars().any(|c| c.is_control()) {
        return Err("caratteri non validi".into());
    }
    s.trim().parse::<Mailbox>().map_err(|e| e.to_string())
}

pub async fn send(cfg: &EmailCfg, password: &str, subject: &str, body: &str) -> Result<(), String> {
    let mut msg = Message::builder()
        .from(parse_mailbox(&cfg.from)?)
        .subject(subject);
    for to in &cfg.to {
        msg = msg.to(parse_mailbox(to)?);
    }
    let msg = msg.body(body.to_string()).map_err(|e| e.to_string())?;

    let builder = match cfg.security {
        Security::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host),
        Security::Starttls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host),
        Security::None => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            &cfg.host,
        )),
    }
    .map_err(|e| e.to_string())?;
    let mut builder = builder
        .port(cfg.port)
        .timeout(Some(std::time::Duration::from_secs(20)));
    if !cfg.user.is_empty() {
        builder = builder.credentials(Credentials::new(cfg.user.clone(), password.to_string()));
    }
    builder
        .build()
        .send(msg)
        .await
        .map(|_| ())
        .map_err(|e| format!("SMTP: {e}"))
}
