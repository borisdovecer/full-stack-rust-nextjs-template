//! The `oauth_signups` table: a provider's verified answer, waiting for the registration form.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct PendingSignup {
    pub provider: String,
    pub subject: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

pub async fn create<'e>(
    ex: impl PgExecutor<'e>,
    token_hash: &[u8],
    signup: &PendingSignup,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO oauth_signups
            (token_hash, provider, subject, email, first_name, last_name, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(token_hash)
    .bind(&signup.provider)
    .bind(&signup.subject)
    .bind(&signup.email)
    .bind(&signup.first_name)
    .bind(&signup.last_name)
    .bind(expires_at)
    .execute(ex)
    .await?;
    Ok(())
}

/// The waiting answer for a token, if it has not expired.
pub async fn find<'e>(
    ex: impl PgExecutor<'e>,
    token_hash: &[u8],
) -> Result<Option<PendingSignup>, sqlx::Error> {
    sqlx::query_as(
        "SELECT provider, subject, email, first_name, last_name FROM oauth_signups
         WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(ex)
    .await
}

/// Takes the waiting answer: returns it and deletes it, so it is used once.
pub async fn take<'e>(
    ex: impl PgExecutor<'e>,
    token_hash: &[u8],
) -> Result<Option<PendingSignup>, sqlx::Error> {
    sqlx::query_as(
        "DELETE FROM oauth_signups WHERE token_hash = $1 AND expires_at > now()
         RETURNING provider, subject, email, first_name, last_name",
    )
    .bind(token_hash)
    .fetch_optional(ex)
    .await
}

/// Deletes up to `limit` sign-ups that expired before `before`.
pub async fn delete_expired_before<'e>(
    ex: impl PgExecutor<'e>,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM oauth_signups WHERE token_hash = ANY(ARRAY(
            SELECT token_hash FROM oauth_signups WHERE expires_at < $1 LIMIT $2))",
    )
    .bind(before)
    .bind(limit)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}
