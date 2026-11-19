//! What the user management routes send and receive.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use app_auth::Permission;

use crate::auth::UserDto;
use crate::auth::repository::users::UserSort;
use crate::auth::repository::{LoginType, Role, UserStatus};
use crate::shared::repository::audit::AuditEvent;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// Matches anywhere in the email or the name.
    pub search: Option<String>,
    pub role: Option<Role>,
    pub status: Option<UserStatus>,
    pub sort: Option<UserSort>,
    /// From 1. Default 1.
    pub page: Option<u32>,
    /// 1 to 100. Default 20.
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserPage {
    pub items: Vec<UserDto>,
    /// How many match, counted up to 10,000. With `total_capped`, at least this many; pages
    /// past it are empty, so narrow the search instead.
    pub total: i64,
    /// More users match than `total` says.
    pub total_capped: bool,
    pub page: u32,
    pub per_page: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEventDto {
    pub id: i64,
    pub actor_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub action: String,
    #[schema(value_type = Object)]
    pub data: Value,
    pub created_at: DateTime<Utc>,
}

impl From<AuditEvent> for AuditEventDto {
    fn from(e: AuditEvent) -> Self {
        Self {
            id: e.id,
            actor_id: e.actor_id,
            actor_email: e.actor_email,
            action: e.action,
            data: e.data,
            created_at: e.created_at,
        }
    }
}

/// A permission granted on top of the role.
#[derive(Debug, Serialize, ToSchema)]
pub struct GrantDto {
    pub permission: Permission,
    /// The email of who granted it; null when that account is gone.
    pub granted_by: Option<String>,
    pub granted_at: DateTime<Utc>,
}

/// One user, what they may do and why, and what happened to their account lately.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserDetail {
    pub user: UserDto,
    /// Everything the user may do: role defaults plus grants.
    pub permissions: Vec<Permission>,
    /// Only the grants on top of the role, with who gave them.
    pub grants: Vec<GrantDto>,
    pub audit: Vec<AuditEventDto>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub bio: Option<String>,
    /// How the account signs in: `password` (default) or `code`.
    pub method: Option<LoginType>,
    /// Required for `password`.
    pub password: Option<String>,
    /// Default `user`.
    pub role: Option<Role>,
    /// Default `active`.
    pub status: Option<UserStatus>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateUserRequest {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    /// An empty bio clears it.
    pub bio: Option<String>,
    pub role: Option<Role>,
    pub status: Option<UserStatus>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPasswordRequest {
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetLoginMethodRequest {
    /// `password` or `code`; see `admin_login_methods` in `GET /api/config`.
    pub method: LoginType,
    /// Required for `password`.
    pub password: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetPermissionsRequest {
    /// The complete list of grants; anything not listed is removed.
    #[schema(value_type = Vec<Permission>)]
    pub permissions: Vec<String>,
}
