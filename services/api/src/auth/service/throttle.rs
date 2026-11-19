//! Login throttling per email and method, and per IP across methods.
//!
//! An attempt is written as a failure *before* its proof is checked, then the failures are
//! counted with it included. Every attempt in flight is therefore already counted, so requests
//! sent at the same moment cannot all slip under the limit: of N parallel attempts, the k-th to
//! be written sees at least k failures.

use chrono::{Duration, Utc};
use serde_json::json;

use app_core::{AppError, AppResult, ErrorCode};

use crate::auth::repository::{LoginType, login_attempts};
use crate::state::AppState;

/// An attempt that passed the limits and still counts as a failure until [`Attempt::succeeded`].
#[derive(Debug)]
#[must_use = "an attempt counts as a failure unless it is marked a success"]
pub struct Attempt {
    id: i64,
}

impl Attempt {
    /// The proof was right: the attempt stops counting, and the email's failures for this
    /// method reset.
    pub async fn succeeded(self, state: &AppState) -> AppResult<()> {
        login_attempts::mark_success(&state.db, self.id).await?;
        Ok(())
    }

    /// The attempt turned out not to be a guess (for a code: no code was live).
    pub async fn discard(self, state: &AppState) -> AppResult<()> {
        login_attempts::delete(&state.db, self.id).await?;
        Ok(())
    }
}

/// Records an attempt, or refuses with `429 too_many_attempts` when the email (for this
/// method) or the IP is over its limit. `details.retry_at` is when the oldest counted failure
/// leaves its window. Codes use their own, tighter limits (`login_code`).
pub async fn begin(
    state: &AppState,
    email: &str,
    method: LoginType,
    ip: Option<&str>,
) -> AppResult<Attempt> {
    let t = &state.config.settings.login_throttle;
    let c = &state.config.settings.login_code;
    let (max_failures, minutes) = match method {
        LoginType::Code => (c.max_attempts, c.pause_minutes),
        _ => (t.max_failures, t.window_minutes),
    };
    let id = login_attempts::record(&state.db, email, method, ip, false).await?;

    let now = Utc::now();
    let email_window = Duration::minutes(i64::from(minutes));
    let by_email =
        login_attempts::email_failures(&state.db, email, method, now - email_window).await?;
    let mut retry_at = (by_email.count > i64::from(max_failures))
        .then_some(by_email.oldest)
        .flatten()
        .map(|oldest| oldest + email_window);

    if let Some(ip) = ip {
        let ip_window = Duration::minutes(i64::from(t.window_minutes));
        let by_ip = login_attempts::ip_failures(&state.db, ip, now - ip_window).await?;
        if by_ip.count > i64::from(t.max_failures_per_ip) {
            retry_at = retry_at.max(by_ip.oldest.map(|oldest| oldest + ip_window));
        }
    }

    match retry_at {
        Some(retry_at) => {
            // A refused attempt was never tried, so it must not extend the pause.
            login_attempts::delete(&state.db, id).await?;
            Err(AppError::new(ErrorCode::TooManyAttempts)
                .with_details(json!({ "retry_at": retry_at })))
        }
        None => Ok(Attempt { id }),
    }
}
