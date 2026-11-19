//! The `login_codes` table. Emails must already be normalised.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

/// What a code is for. A code only works for the purpose it was sent for.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, Serialize, Deserialize, utoipa::ToSchema,
)]
#[sqlx(type_name = "login_code_purpose", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum CodePurpose {
    /// Sign in to an account that uses codes.
    Login,
    /// Prove the inbox while creating an account.
    Register,
    /// Prove the inbox while switching an account to codes.
    Change,
}

#[derive(Debug, Clone)]
pub struct NewCode<'a> {
    pub id: Uuid,
    pub email: &'a str,
    pub purpose: CodePurpose,
    pub code_hash: &'a [u8],
    pub ip: Option<&'a str>,
    pub expires_at: DateTime<Utc>,
}

/// A code that can still be tried.
#[derive(Clone, sqlx::FromRow)]
pub struct LiveCode {
    pub id: Uuid,
    pub code_hash: Vec<u8>,
    pub attempts: i32,
}

impl std::fmt::Debug for LiveCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveCode")
            .field("id", &self.id)
            .field("attempts", &self.attempts)
            .finish_non_exhaustive()
    }
}

/// Stores a code and retires the older unused ones for the same email and purpose, so only
/// the newest code works.
pub async fn create(conn: &mut PgConnection, new: NewCode<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE login_codes SET used_at = now()
         WHERE email = $1 AND purpose = $2 AND used_at IS NULL",
    )
    .bind(new.email)
    .bind(new.purpose)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO login_codes (id, email, purpose, code_hash, ip, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(new.id)
    .bind(new.email)
    .bind(new.purpose)
    .bind(new.code_hash)
    .bind(new.ip)
    .bind(new.expires_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// When the newest unused code for this email and purpose was made.
pub async fn last_unused_created_at<'e>(
    ex: impl PgExecutor<'e>,
    email: &str,
    purpose: CodePurpose,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT max(created_at) FROM login_codes
         WHERE email = $1 AND purpose = $2 AND used_at IS NULL",
    )
    .bind(email)
    .bind(purpose)
    .fetch_one(ex)
    .await
}

/// Codes asked for from one IP since `since`, and the oldest of them.
pub async fn requests_from_ip<'e>(
    ex: impl PgExecutor<'e>,
    ip: &str,
    since: DateTime<Utc>,
) -> Result<(i64, Option<DateTime<Utc>>), sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*), min(created_at) FROM login_codes WHERE ip = $1 AND created_at > $2",
    )
    .bind(ip)
    .bind(since)
    .fetch_one(ex)
    .await
}

/// Takes one try on the newest unused, unexpired code for this email and purpose, if it has
/// fewer than `max_attempts` used, and returns it to compare against. One statement, so
/// parallel requests can never take more tries than the code has.
pub async fn take_try<'e>(
    ex: impl PgExecutor<'e>,
    email: &str,
    purpose: CodePurpose,
    max_attempts: i32,
) -> Result<Option<LiveCode>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE login_codes SET attempts = attempts + 1
         WHERE id = (
             SELECT id FROM login_codes
             WHERE email = $1 AND purpose = $2 AND used_at IS NULL AND expires_at > now()
             ORDER BY created_at DESC
             LIMIT 1)
           AND attempts < $3
         RETURNING id, code_hash, attempts",
    )
    .bind(email)
    .bind(purpose)
    .bind(max_attempts)
    .fetch_optional(ex)
    .await
}

/// Spends a code. False when another request spent it first.
pub async fn mark_used<'e>(ex: impl PgExecutor<'e>, id: Uuid) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("UPDATE login_codes SET used_at = now() WHERE id = $1 AND used_at IS NULL")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(r.rows_affected() == 1)
}

/// Deletes up to `limit` codes that expired before `before`.
pub async fn delete_expired_before<'e>(
    ex: impl PgExecutor<'e>,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM login_codes WHERE id = ANY(ARRAY(
            SELECT id FROM login_codes WHERE expires_at < $1 LIMIT $2))",
    )
    .bind(before)
    .bind(limit)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}
