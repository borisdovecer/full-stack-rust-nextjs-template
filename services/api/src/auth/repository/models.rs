//! Rows and the Postgres enums they use.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, Serialize, Deserialize, utoipa::ToSchema,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Admin,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, Serialize, Deserialize, utoipa::ToSchema,
)]
#[sqlx(type_name = "user_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum UserStatus {
    /// Registered, waiting for an admin (registration mode `approval`).
    Pending,
    Active,
    /// Cannot sign in; existing sessions are revoked.
    Suspended,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, Serialize, Deserialize, utoipa::ToSchema,
)]
#[sqlx(type_name = "theme", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type, Serialize, Deserialize, utoipa::ToSchema,
)]
#[sqlx(type_name = "login_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum LoginType {
    Password,
    /// A Solana wallet signs a one-time message; the address is in `users.wallet`.
    Wallet,
    /// A one-time code sent to the email.
    Code,
    /// Google or GitHub.
    External,
}

/// A user without the password hash. This is the type to pass around.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub bio: Option<String>,
    pub login_type: LoginType,
    pub wallet: Option<String>,
    pub role: Role,
    pub status: UserStatus,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A user with the password hash, read only where a password is checked.
#[derive(sqlx::FromRow)]
pub struct Credentials {
    #[sqlx(flatten)]
    pub user: User,
    /// Null for an account that does not sign in with a password.
    pub password_hash: Option<String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("user", &self.user)
            .field("password_hash", &"***")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Preferences {
    pub theme: Theme,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

/// A valid session and its user, read in one query on every authenticated request.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SessionWithUser {
    pub session_id: Uuid,
    pub session_created_at: DateTime<Utc>,
    pub session_last_seen_at: DateTime<Utc>,
    pub session_expires_at: DateTime<Utc>,
    #[sqlx(flatten)]
    pub user: User,
}
