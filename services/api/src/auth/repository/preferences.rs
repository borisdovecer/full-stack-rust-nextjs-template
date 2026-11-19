//! The `user_preferences` table: one row per user, created with the user.

use sqlx::PgExecutor;
use uuid::Uuid;

use super::models::{Preferences, Theme};

pub async fn get<'e>(ex: impl PgExecutor<'e>, user_id: Uuid) -> Result<Preferences, sqlx::Error> {
    sqlx::query_as("SELECT theme, updated_at FROM user_preferences WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(ex)
        .await
}

/// `None` leaves a field as it is.
#[derive(Debug, Clone, Copy, Default)]
pub struct PreferencesPatch {
    pub theme: Option<Theme>,
}

pub async fn update<'e>(
    ex: impl PgExecutor<'e>,
    user_id: Uuid,
    patch: PreferencesPatch,
) -> Result<Preferences, sqlx::Error> {
    sqlx::query_as(
        "UPDATE user_preferences SET theme = COALESCE($2, theme)
         WHERE user_id = $1
         RETURNING theme, updated_at",
    )
    .bind(user_id)
    .bind(patch.theme)
    .fetch_one(ex)
    .await
}
