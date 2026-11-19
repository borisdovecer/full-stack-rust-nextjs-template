//! Mailed sign-in codes: making and sending one, and spending one.
//!
//! "Send me a code" answers the same whether or not the address has an account, and the mail
//! leaves in the background so the answer also takes the same time. A wrong guess counts only
//! against a live code, so nobody can pause an address by posting codes at it. A try is taken
//! from the code in the same statement that checks one is left, so parallel guesses cannot
//! exceed `login_code.max_attempts`.

use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use app_auth::code;
use app_core::{AppError, AppResult, ErrorCode};
use app_mail::Message;

use super::throttle;
use crate::auth::repository::LoginType;
use crate::auth::repository::login_codes::{self, CodePurpose, NewCode};
use crate::state::AppState;

/// Makes a code for `email` and, when `send` is true, mails it. Does nothing when a code went to
/// the address less than `resend_seconds` ago. Refuses only when the IP asked for too many.
pub async fn issue(
    state: &AppState,
    email: &str,
    purpose: CodePurpose,
    ip: Option<&str>,
    send: bool,
) -> AppResult<()> {
    let s = &state.config.settings.login_code;
    let now = Utc::now();
    let lifetime = Duration::minutes(i64::from(s.lifetime_minutes));

    if let Some(ip) = ip {
        let (count, oldest) = login_codes::requests_from_ip(&state.db, ip, now - lifetime).await?;
        if count >= i64::from(s.max_requests_per_ip) {
            let retry_at = oldest.unwrap_or(now) + lifetime;
            return Err(AppError::new(ErrorCode::TooManyAttempts)
                .with_details(json!({ "retry_at": retry_at })));
        }
    }
    let resend = Duration::seconds(i64::from(s.resend_seconds));
    let last = login_codes::last_unused_created_at(&state.db, email, purpose).await?;
    if last.is_some_and(|last| last > now - resend) {
        return Ok(());
    }

    let id = Uuid::new_v4();
    let plain = code::generate();
    let hash = code::hash(id.as_bytes(), &plain);
    let mut conn = state.db.acquire().await?;
    login_codes::create(
        &mut conn,
        NewCode {
            id,
            email,
            purpose,
            code_hash: &hash,
            ip,
            expires_at: now + lifetime,
        },
    )
    .await?;

    if send {
        let message = message(state, email, purpose, &plain);
        let mailer = state.mailer.clone();
        let db = state.db.clone();
        tokio::spawn(async move {
            if let Err(e) = mailer.send(message).await {
                tracing::warn!(error = %e, code_id = %id, "sign-in code not sent");
                // A code nobody received must not hold back the next request.
                if let Err(e) = login_codes::mark_used(&db, id).await {
                    tracing::warn!(error = %e, code_id = %id, "cannot retire an unsent code");
                }
            }
        });
    }
    Ok(())
}

/// Spends the newest live code for `email` and `purpose`. True when `given` is that code.
pub async fn spend(
    state: &AppState,
    email: &str,
    purpose: CodePurpose,
    given: &str,
    ip: Option<&str>,
) -> AppResult<bool> {
    let attempt = throttle::begin(state, email, LoginType::Code, ip).await?;
    let max = i32::try_from(state.config.settings.login_code.max_attempts).unwrap_or(i32::MAX);
    let Some(live) = login_codes::take_try(&state.db, email, purpose, max).await? else {
        attempt.discard(state).await?;
        return Ok(false);
    };
    let given = given.trim();
    if code::looks_valid(given)
        && code::verify(live.id.as_bytes(), given, &live.code_hash)
        && login_codes::mark_used(&state.db, live.id).await?
    {
        attempt.succeeded(state).await?;
        return Ok(true);
    }
    Ok(false)
}

fn message(state: &AppState, email: &str, purpose: CodePurpose, plain: &str) -> Message {
    let app = &state.config.settings.app.name;
    let minutes = state.config.settings.login_code.lifetime_minutes;
    let (subject, what) = match purpose {
        CodePurpose::Login => (format!("Your sign-in code for {app}"), "sign in"),
        CodePurpose::Register => (
            format!("Your code to create your {app} account"),
            "create your account",
        ),
        CodePurpose::Change => (
            format!("Your code to change how you sign in to {app}"),
            "change how you sign in",
        ),
    };
    Message {
        to: email.to_string(),
        subject,
        text: format!(
            "Your code is {plain}\n\nEnter it to {what}. It works once, for {minutes} minutes.\n\n\
             If you did not ask for it, you can ignore this email."
        ),
    }
}
