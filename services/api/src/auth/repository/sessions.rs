//! The `user_sessions` table. Only token hashes are stored here.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use uuid::Uuid;

use super::models::{Session, SessionWithUser};
use super::users::user_columns;

#[derive(Debug, Clone)]
pub struct NewSession<'a> {
    pub user_id: Uuid,
    pub token_hash: &'a [u8],
    pub expires_at: DateTime<Utc>,
    pub ip: Option<&'a str>,
    pub user_agent: Option<&'a str>,
}

pub async fn create<'e>(
    ex: impl PgExecutor<'e>,
    new: NewSession<'_>,
) -> Result<Session, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO user_sessions (user_id, token_hash, expires_at, ip, user_agent)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, user_id, created_at, last_seen_at, expires_at, ip, user_agent",
    )
    .bind(new.user_id)
    .bind(new.token_hash)
    .bind(new.expires_at)
    .bind(new.ip)
    .bind(new.user_agent)
    .fetch_one(ex)
    .await
}

/// The session for a token hash, with its user, if it is not revoked and not expired.
pub async fn find_valid<'e>(
    ex: impl PgExecutor<'e>,
    token_hash: &[u8],
) -> Result<Option<SessionWithUser>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT s.id AS session_id, s.created_at AS session_created_at,
                s.last_seen_at AS session_last_seen_at,
                s.expires_at AS session_expires_at, ",
        user_columns!("u"),
        "
         FROM user_sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.revoked_at IS NULL AND s.expires_at > now()"
    ))
    .bind(token_hash)
    .fetch_optional(ex)
    .await
}

/// Records use of a session and, when given, moves its expiry.
pub async fn touch<'e>(
    ex: impl PgExecutor<'e>,
    id: Uuid,
    new_expires_at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE user_sessions SET last_seen_at = now(), expires_at = COALESCE($2, expires_at)
         WHERE id = $1",
    )
    .bind(id)
    .bind(new_expires_at)
    .execute(ex)
    .await?;
    Ok(())
}

/// Revokes one session of one user. False when it was not theirs or already ended.
pub async fn revoke<'e>(
    ex: impl PgExecutor<'e>,
    id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE user_sessions SET revoked_at = now()
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(id)
    .bind(user_id)
    .execute(ex)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Revokes every live session of a user, except `keep` when given. Returns how many.
pub async fn revoke_all<'e>(
    ex: impl PgExecutor<'e>,
    user_id: Uuid,
    keep: Option<Uuid>,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE user_sessions SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
           AND ($2::uuid IS NULL OR id <> $2)",
    )
    .bind(user_id)
    .bind(keep)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}

/// A user's live sessions, most recently used first.
pub async fn list_active<'e>(
    ex: impl PgExecutor<'e>,
    user_id: Uuid,
) -> Result<Vec<Session>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, user_id, created_at, last_seen_at, expires_at, ip, user_agent
         FROM user_sessions
         WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
         ORDER BY last_seen_at DESC",
    )
    .bind(user_id)
    .fetch_all(ex)
    .await
}

/// Deletes up to `limit` sessions that expired before `before`.
pub async fn delete_expired<'e>(
    ex: impl PgExecutor<'e>,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM user_sessions WHERE id = ANY(ARRAY(
            SELECT id FROM user_sessions WHERE expires_at < $1 LIMIT $2))",
    )
    .bind(before)
    .bind(limit)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}

/// Deletes up to `limit` sessions revoked before `before`. Separate from [`delete_expired`] so
/// each uses its own index (`OR` would read the whole table).
pub async fn delete_revoked<'e>(
    ex: impl PgExecutor<'e>,
    before: DateTime<Utc>,
    limit: i64,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM user_sessions WHERE id = ANY(ARRAY(
            SELECT id FROM user_sessions WHERE revoked_at < $1 LIMIT $2))",
    )
    .bind(before)
    .bind(limit)
    .execute(ex)
    .await?;
    Ok(r.rows_affected())
}
