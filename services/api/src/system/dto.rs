//! What the system routes send.

use serde::Serialize;
use utoipa::ToSchema;

use app_core::config::{AppEnv, RegistrationMode};

use crate::auth::repository::LoginType;

#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    /// `ok` when every check passes, `degraded` otherwise (answered with 503).
    pub status: &'static str,
    pub version: &'static str,
    pub commit: &'static str,
    pub environment: AppEnv,
    /// `ok` or `down`.
    pub database: &'static str,
    /// `current`, `pending`, `failed`, `changed`, `unknown`, or `unchecked` when the database
    /// is down.
    pub migrations: &'static str,
    /// Where mail goes: `resend`, or `log` when no key is set (codes are then written to the
    /// API log, so a server should never say `log`).
    pub mail: &'static str,
}

/// What the web needs to know before anyone signs in.
#[derive(Debug, Serialize, ToSchema)]
pub struct PublicConfig {
    pub app_name: String,
    pub registration_mode: RegistrationMode,
    /// The sign-in methods this server offers, in the order to show them.
    pub login_methods: Vec<LoginType>,
    /// The methods an admin may give an account when creating it or recovering it.
    pub admin_login_methods: Vec<LoginType>,
    /// The providers of the `external` method that are on: `google`, `github`.
    pub oauth_providers: Vec<&'static str>,
    pub version: &'static str,
}
