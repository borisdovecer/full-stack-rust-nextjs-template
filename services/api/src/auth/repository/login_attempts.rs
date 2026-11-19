//! The `auth_login_attempts` table, read for throttling.
//!
//! An attempt is written as a failure before its proof is checked and marked a success after,
//! so an attempt still in flight already counts against the limits.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

use super::models::LoginType;

/// Writes an attempt and returns its id.
pub async fn record<'e>(
    ex: impl PgExecutor<'e>,
    email: &str,
    method: LoginType,
    ip: Option<&str>,
    success: bool,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO auth_login_attempts (email, method, ip, success) VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(email)
    .bind(method)
    .bind(ip)
    .bind(success)
    .fetch_one(ex)
    .await
}

/// Turns a recorded failure into a success.
pub async fn mark_success<'e>(ex: impl PgExecutor<'e>, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE auth_login_attempts SET success = true WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

/// Removes an attempt that turned out not to count.
pub async fn delete<'e>(ex: impl PgExecutor<'e>, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM auth_login_attempts WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

/// Failures since `since` and the oldest of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct Failures {
    pub count: i64,
    pub oldest: Option<DateTime<Utc>>,
}

/// Failures for an email with one method since `since`, not counting those before that
/// method's last success.
pub async fn email_failures<'e>(
    ex: impl PgExecutor<'e>,
    email: &str,
    method: LoginType,
    since: DateTime<Utc>,
) -> Result<Failures, sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*) AS count, min(attempted_at) AS oldest
         FROM auth_login_attempts
         WHERE email = $1 AND method = $2 AND NOT success
           AND attempted_at > GREATEST($3, COALESCE(
                (SELECT max(attempted_at) FROM auth_login_attempts
                  WHERE email = $1 AND method = $2 AND success),
                $3))",
    )
    .bind(email)
    .bind(method)
    .bind(since)
    .fetch_one(ex)
    .await
}

/// Failures from an IP since `since`, across all emails.
pub async fn ip_failures<'e>(
    ex: impl PgExecutor<'e>,
    ip: &str,
    since: DateTime<Utc>,
) -> Result<Failures, sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*) AS count, min(attempted_at) AS oldest
         FROM auth_login_attempts
         WHERE ip = $1 AND NOT success AND attempted_at > $2",
    )
    .bind(ip)
    .bind(since)
    .fetch_one(ex)
    .await
}

/// Deletes up to `limit` attempts older than `before`; call until it returns less than `limit`.
/// Every batched delete uses `= ANY(ARRAY(...))`: an `IN (subquery)` may read the whole table.
pub async fn delete_before<'e>(
    ex: impl PgExecutor<'e>,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM auth_login_attempts WHERE id = ANY(ARRAY(
            SELECT id FROM auth_login_attempts WHERE attempted_at < $1 LIMIT $2))",
    )
    .bind(before)
    .bind(limit)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}
