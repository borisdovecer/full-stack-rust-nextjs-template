//! Background clean-up, once an hour: ended sessions, old login attempts, expired codes,
//! wallet challenges and provider sign-ups.
//!
//! Every API instance runs the timer, but an advisory lock lets only one of them clean up at a
//! time. Rows are deleted in batches, so a large backlog never holds one long lock.

use std::future::Future;
use std::time::Duration;

use chrono::{DateTime, Utc};

use app_db::Pool;

use crate::auth::oauth::repository as oauth_signups;
use crate::auth::repository::{login_attempts, login_codes, sessions, wallet_nonces};

const EVERY: Duration = Duration::from_secs(60 * 60);
const KEEP_ENDED_SESSIONS_DAYS: i64 = 1;
const KEEP_LOGIN_ATTEMPTS_DAYS: i64 = 30;
const KEEP_EXPIRED_CODES_DAYS: i64 = 1;
/// Rows per delete statement.
const BATCH: i64 = 5_000;
/// The advisory lock key the clean-up holds while it runs. Any fixed number no other code uses.
pub const LOCK_KEY: i64 = 0x6170_705f_686b; // "app_hk"

pub fn spawn(db: Pool) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(EVERY);
        loop {
            tick.tick().await;
            run_once(&db).await;
        }
    });
}

/// Runs one clean-up, unless another instance is running one. Returns whether it ran.
pub async fn run_once(db: &Pool) -> bool {
    let mut lock_conn = match db.acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "clean-up skipped: no database connection");
            return false;
        }
    };
    match app_db::try_advisory_lock(&mut lock_conn, LOCK_KEY).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::debug!("clean-up skipped: another instance is running it");
            return false;
        }
        Err(e) => {
            tracing::warn!(error = %e, "clean-up skipped: cannot take the lock");
            return false;
        }
    }

    let now = Utc::now();
    let ended = now - chrono::Duration::days(KEEP_ENDED_SESSIONS_DAYS);
    let attempts = now - chrono::Duration::days(KEEP_LOGIN_ATTEMPTS_DAYS);
    let expired = now - chrono::Duration::days(KEEP_EXPIRED_CODES_DAYS);

    batched("expired sessions", ended, |t| {
        sessions::delete_expired(db, t, BATCH)
    })
    .await;
    batched("revoked sessions", ended, |t| {
        sessions::delete_revoked(db, t, BATCH)
    })
    .await;
    batched("old login attempts", attempts, |t| {
        login_attempts::delete_before(db, t, BATCH)
    })
    .await;
    batched("expired sign-in codes", expired, |t| {
        login_codes::delete_expired_before(db, t, BATCH)
    })
    .await;
    batched("expired wallet challenges", expired, |t| {
        wallet_nonces::delete_expired_before(db, t, BATCH)
    })
    .await;
    batched("expired provider sign-ups", expired, |t| {
        oauth_signups::delete_expired_before(db, t, BATCH)
    })
    .await;

    if let Err(e) = app_db::advisory_unlock(&mut lock_conn, LOCK_KEY).await {
        tracing::warn!(error = %e, "cannot release the clean-up lock");
    }
    true
}

/// Calls `delete` until a batch comes back short, and logs the total.
async fn batched<F, Fut>(what: &str, before: DateTime<Utc>, delete: F)
where
    F: Fn(DateTime<Utc>) -> Fut,
    Fut: Future<Output = Result<u64, sqlx::Error>>,
{
    let mut total = 0u64;
    loop {
        match delete(before).await {
            Ok(n) => {
                total += n;
                if n < BATCH.unsigned_abs() {
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, what, "clean-up failed");
                break;
            }
        }
    }
    if total > 0 {
        tracing::info!(deleted = total, what, "clean-up removed rows");
    }
}
