//! The `audit_events` table. Write in the same transaction as the change it records.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgExecutor;
use uuid::Uuid;

/// Action names. Strings in the database, so a new one needs no migration.
pub mod action {
    pub const USER_CREATED: &str = "user.created";
    pub const USER_REGISTERED: &str = "user.registered";
    pub const USER_UPDATED: &str = "user.updated";
    pub const USER_PASSWORD_SET: &str = "user.password_set";
    pub const USER_PASSWORD_CHANGED: &str = "user.password_changed";
    pub const USER_LOGIN_METHOD_CHANGED: &str = "user.login_method_changed";
    pub const PERMISSIONS_CHANGED: &str = "user.permissions_changed";
}

pub async fn record<'e>(
    ex: impl PgExecutor<'e>,
    actor_id: Option<Uuid>,
    subject_id: Option<Uuid>,
    action: &str,
    data: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events (actor_id, subject_id, action, data) VALUES ($1, $2, $3, $4)",
    )
    .bind(actor_id)
    .bind(subject_id)
    .bind(action)
    .bind(data)
    .execute(ex)
    .await?;
    Ok(())
}

/// The newest events about a user, with the actor's email.
pub async fn list_for_subject<'e>(
    ex: impl PgExecutor<'e>,
    subject_id: Uuid,
    limit: i64,
) -> Result<Vec<AuditEvent>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.id, e.actor_id, a.email AS actor_email, e.subject_id, e.action, e.data,
                e.created_at
         FROM audit_events e LEFT JOIN users a ON a.id = e.actor_id
         WHERE e.subject_id = $1
         ORDER BY e.created_at DESC, e.id DESC
         LIMIT $2",
    )
    .bind(subject_id)
    .bind(limit)
    .fetch_all(ex)
    .await
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: i64,
    pub actor_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub subject_id: Option<Uuid>,
    pub action: String,
    pub data: Value,
    pub created_at: DateTime<Utc>,
}
