//! The `user_identities` table: which Google or GitHub account an `external` account uses.

use sqlx::PgExecutor;
use uuid::Uuid;

use super::models::User;
use super::users::user_columns;

/// The account tied to this provider account.
pub async fn find_user<'e>(
    ex: impl PgExecutor<'e>,
    provider: &str,
    subject: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!("u"),
        " FROM user_identities i JOIN users u ON u.id = i.user_id
          WHERE i.provider = $1 AND i.subject = $2"
    ))
    .bind(provider)
    .bind(subject)
    .fetch_optional(ex)
    .await
}

/// The provider an account is tied to, if any.
pub async fn provider_of<'e>(
    ex: impl PgExecutor<'e>,
    user_id: Uuid,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT provider FROM user_identities WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(ex)
        .await
}

/// Ties an account to a provider account. Fails on `user_identities_pkey` when the provider
/// account is tied to another account.
pub async fn insert<'e>(
    ex: impl PgExecutor<'e>,
    user_id: Uuid,
    provider: &str,
    subject: &str,
    email: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_identities (user_id, provider, subject, email) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(provider)
    .bind(subject)
    .bind(email)
    .execute(ex)
    .await?;
    Ok(())
}

/// The constraint whose violation means "this provider account belongs to another account".
pub const IDENTITY_KEY: &str = "user_identities_pkey";
