//! What the `/api/me` routes send and receive.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::repository::models::Session;
use crate::auth::repository::{LoginType, Theme};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SessionDto {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    /// True for the session making this request.
    pub current: bool,
}

impl SessionDto {
    pub fn new(s: Session, current: Uuid) -> Self {
        Self {
            current: s.id == current,
            id: s.id,
            created_at: s.created_at,
            last_seen_at: s.last_seen_at,
            expires_at: s.expires_at,
            ip: s.ip,
            user_agent: s.user_agent,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMeRequest {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    /// An empty bio clears it.
    pub bio: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeLoginMethodRequest {
    /// The method to sign in with from now on. One of `login_methods` in `GET /api/config`.
    pub method: LoginType,
    /// The new password, for `password`.
    pub password: Option<String>,
    /// For `code`: the code sent by `POST /api/me/login-code`.
    pub login_code: Option<String>,
    /// For `wallet`: the new wallet's address, and the challenge from
    /// `POST /api/auth/wallet/nonce` as it signed it (`message`, `signature`, base64).
    pub address: Option<String>,
    pub message: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePreferencesRequest {
    pub theme: Option<Theme>,
}
