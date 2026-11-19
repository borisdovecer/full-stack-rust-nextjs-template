//! The `user_permissions` table: grants on top of a role. Names are checked by the API against
//! its fixed list before they reach this module.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

/// One grant, with who gave it and when.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Grant {
    pub permission: String,
    /// The granter's email; `None` when they were deleted or it was the CLI.
    pub granted_by: Option<String>,
    pub granted_at: DateTime<Utc>,
}

/// The user's grant names, sorted as strings.
pub async fn list<'e>(ex: impl PgExecutor<'e>, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT permission FROM user_permissions WHERE user_id = $1 ORDER BY permission",
    )
    .bind(user_id)
    .fetch_all(ex)
    .await
}

/// The user's grants with who gave them, sorted by name.
pub async fn list_grants<'e>(
    ex: impl PgExecutor<'e>,
    user_id: Uuid,
) -> Result<Vec<Grant>, sqlx::Error> {
    sqlx::query_as(
        "SELECT p.permission, g.email AS granted_by, p.granted_at
         FROM user_permissions p LEFT JOIN users g ON g.id = p.granted_by
         WHERE p.user_id = $1
         ORDER BY p.permission",
    )
    .bind(user_id)
    .fetch_all(ex)
    .await
}

/// Replaces the user's grants with `permissions`. Grants that stay keep their granter and time.
/// Call inside a transaction.
pub async fn replace(
    conn: &mut PgConnection,
    user_id: Uuid,
    permissions: &[String],
    granted_by: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_permissions WHERE user_id = $1 AND NOT (permission = ANY($2))")
        .bind(user_id)
        .bind(permissions)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO user_permissions (user_id, permission, granted_by)
         SELECT $1, p, $3 FROM unnest($2::varchar[]) AS p
         ON CONFLICT (user_id, permission) DO NOTHING",
    )
    .bind(user_id)
    .bind(permissions)
    .bind(granted_by)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
