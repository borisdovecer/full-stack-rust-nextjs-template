//! Sends email. One transport is chosen at start: Resend's HTTP API when a key is set, the log
//! otherwise (development), or an in-memory outbox (tests).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

const RESEND_URL: &str = "https://api.resend.com/emails";
const TIMEOUT: Duration = Duration::from_secs(10);

/// One plain-text message to one address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("the mail request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("Resend refused the message with status {status}")]
    Refused { status: u16 },
}

/// Messages a test mailer has "sent", newest last.
pub type Outbox = Arc<Mutex<Vec<Message>>>;

#[derive(Clone)]
pub enum Mailer {
    Resend(Resend),
    /// Writes each message, code included, to the log. Never use it on a server.
    Log,
    Capture(Outbox),
}

#[derive(Clone)]
pub struct Resend {
    client: reqwest::Client,
    api_key: String,
    from: String,
    url: String,
}

impl std::fmt::Debug for Mailer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.mode())
    }
}

impl Mailer {
    /// Resend with `api_key`, sending as `from` (`Name <address>`).
    pub fn resend(api_key: &str, from: &str) -> Result<Self, MailError> {
        Self::resend_at(api_key, from, RESEND_URL)
    }

    /// Resend at another address; tests point it at a local fake.
    pub fn resend_at(api_key: &str, from: &str, url: &str) -> Result<Self, MailError> {
        let client = reqwest::Client::builder().timeout(TIMEOUT).build()?;
        Ok(Mailer::Resend(Resend {
            client,
            api_key: api_key.to_string(),
            from: from.to_string(),
            url: url.to_string(),
        }))
    }

    /// A mailer that keeps messages in memory, and the outbox to read them from.
    pub fn capture() -> (Self, Outbox) {
        let outbox = Outbox::default();
        (Mailer::Capture(outbox.clone()), outbox)
    }

    /// `resend`, `log` or `capture`, as `/api/health` reports it.
    pub fn mode(&self) -> &'static str {
        match self {
            Mailer::Resend(_) => "resend",
            Mailer::Log => "log",
            Mailer::Capture(_) => "capture",
        }
    }

    /// Whether messages reach a real inbox.
    pub fn delivers(&self) -> bool {
        matches!(self, Mailer::Resend(_))
    }

    pub async fn send(&self, message: Message) -> Result<(), MailError> {
        match self {
            Mailer::Resend(r) => {
                let res = r
                    .client
                    .post(&r.url)
                    .bearer_auth(&r.api_key)
                    .json(&json!({
                        "from": r.from,
                        "to": [message.to],
                        "subject": message.subject,
                        "text": message.text,
                    }))
                    .send()
                    .await?;
                if res.status().is_success() {
                    Ok(())
                } else {
                    Err(MailError::Refused {
                        status: res.status().as_u16(),
                    })
                }
            }
            Mailer::Log => {
                tracing::info!(
                    to = %message.to,
                    subject = %message.subject,
                    text = %message.text,
                    "mail not sent: no RESEND_API_KEY, written here instead"
                );
                Ok(())
            }
            Mailer::Capture(outbox) => {
                if let Ok(mut out) = outbox.lock() {
                    out.push(message);
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use axum::Json;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use serde_json::Value;

    fn message() -> Message {
        Message {
            to: "ana@example.com".into(),
            subject: "Your code".into(),
            text: "123456".into(),
        }
    }

    async fn fake_resend(status: StatusCode) -> (String, Arc<Mutex<Vec<(String, Value)>>>) {
        let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let store = seen.clone();
        let app = axum::Router::new().route(
            "/emails",
            post(move |headers: HeaderMap, Json(body): Json<Value>| {
                let store = store.clone();
                async move {
                    let auth = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    store.lock().unwrap().push((auth, body));
                    status
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await });
        (format!("http://{addr}/emails"), seen)
    }

    #[tokio::test]
    async fn resend_gets_the_key_sender_and_message() {
        let (url, seen) = fake_resend(StatusCode::OK).await;
        let mailer = Mailer::resend_at("re_test", "App <no-reply@example.com>", &url).unwrap();
        mailer.send(message()).await.unwrap();
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0].0, "Bearer re_test");
        assert_eq!(
            seen[0].1,
            json!({
                "from": "App <no-reply@example.com>",
                "to": ["ana@example.com"],
                "subject": "Your code",
                "text": "123456",
            })
        );
    }

    #[tokio::test]
    async fn a_refusal_is_an_error() {
        let (url, _) = fake_resend(StatusCode::UNPROCESSABLE_ENTITY).await;
        let mailer = Mailer::resend_at("re_test", "a@example.com", &url).unwrap();
        assert!(matches!(
            mailer.send(message()).await,
            Err(MailError::Refused { status: 422 })
        ));
    }

    #[tokio::test]
    async fn capture_keeps_messages_and_hides_nothing_else() {
        let (mailer, outbox) = Mailer::capture();
        mailer.send(message()).await.unwrap();
        assert_eq!(outbox.lock().unwrap().as_slice(), [message()]);
        assert_eq!(mailer.mode(), "capture");
        assert!(!mailer.delivers());
        assert_eq!(format!("{mailer:?}"), "capture");
    }
}
