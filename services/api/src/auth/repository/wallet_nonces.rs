//! The `wallet_nonces` table: challenges a Solana wallet signs.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

/// Stores a nonce and returns when it was issued.
pub async fn create<'e>(
    ex: impl PgExecutor<'e>,
    nonce: &str,
    address: &str,
    ip: Option<&str>,
    expires_at: DateTime<Utc>,
) -> Result<DateTime<Utc>, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO wallet_nonces (nonce, address, ip, expires_at) VALUES ($1, $2, $3, $4)
         RETURNING issued_at",
    )
    .bind(nonce)
    .bind(address)
    .bind(ip)
    .bind(expires_at)
    .fetch_one(ex)
    .await
}

/// Nonces issued to one IP since `since`, and the oldest of them.
pub async fn issued_to_ip<'e>(
    ex: impl PgExecutor<'e>,
    ip: &str,
    since: DateTime<Utc>,
) -> Result<(i64, Option<DateTime<Utc>>), sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*), min(issued_at) FROM wallet_nonces WHERE ip = $1 AND issued_at > $2",
    )
    .bind(ip)
    .bind(since)
    .fetch_one(ex)
    .await
}

/// Spends a nonce issued for `address` that is unused and unexpired. False otherwise.
pub async fn consume<'e>(
    ex: impl PgExecutor<'e>,
    nonce: &str,
    address: &str,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE wallet_nonces SET used_at = now()
         WHERE nonce = $1 AND address = $2 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(nonce)
    .bind(address)
    .execute(ex)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Deletes up to `limit` nonces that expired before `before`.
pub async fn delete_expired_before<'e>(
    ex: impl PgExecutor<'e>,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM wallet_nonces WHERE nonce = ANY(ARRAY(
            SELECT nonce FROM wallet_nonces WHERE expires_at < $1 LIMIT $2))",
    )
    .bind(before)
    .bind(limit)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}
