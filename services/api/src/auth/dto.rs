//! What the auth routes send and receive, and the account types other scopes send too. Rows
//! are never serialised directly, so a new column cannot leak by accident.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use app_auth::Permission;

use super::repository::login_codes::CodePurpose;
use super::repository::models::Preferences;
use super::repository::{LoginType, Role, Theme, User, UserStatus};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PreferencesDto {
    pub theme: Theme,
}

impl From<Preferences> for PreferencesDto {
    fn from(p: Preferences) -> Self {
        Self { theme: p.theme }
    }
}

/// The signed-in user, what they may do, and their preferences.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Me {
    pub user: UserDto,
    pub permissions: Vec<Permission>,
    pub preferences: PreferencesDto,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    /// How the account will sign in. Default `password`.
    pub method: Option<LoginType>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub bio: Option<String>,
    /// Required for `password`.
    pub password: Option<String>,
    /// Required for `code`: the code sent by `POST /api/auth/code` with purpose `register`.
    pub login_code: Option<String>,
    /// For `wallet`: the Solana address, and the challenge from `POST /api/auth/wallet/nonce`
    /// as the wallet signed it (`message`) with its `signature`, both base64.
    pub address: Option<String>,
    pub message: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// Default `password`.
    pub method: Option<LoginType>,
    pub email: String,
    /// For `password`.
    pub password: Option<String>,
    /// For `code`: the code sent by `POST /api/auth/code`.
    pub login_code: Option<String>,
    /// For `wallet`: the address, and the signed challenge (`message`, `signature`, base64).
    pub address: Option<String>,
    pub message: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct WalletNonceRequest {
    /// The Solana address that will sign, base58.
    pub address: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CodeRequest {
    pub email: String,
    /// `login` (default) or `register`. A code works only for its purpose.
    pub purpose: Option<CodePurpose>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub bio: Option<String>,
    /// How the account signs in.
    pub login_type: LoginType,
    /// The Solana address, for a `wallet` account.
    pub wallet: Option<String>,
    pub role: Role,
    pub status: UserStatus,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            first_name: u.first_name,
            last_name: u.last_name,
            bio: u.bio,
            login_type: u.login_type,
            wallet: u.wallet,
            role: u.role,
            status: u.status,
            last_login_at: u.last_login_at,
            created_at: u.created_at,
        }
    }
}
