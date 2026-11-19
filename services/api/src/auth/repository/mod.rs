//! The tables `auth` owns. Other scopes reach them through `crate::auth::repository`.
//!
//! Functions that run one statement take any executor (`&PgPool` or `&mut *tx`). Functions that
//! must run several statements together take `&mut PgConnection`, so the caller decides the
//! transaction.

pub mod identities;
pub mod login_attempts;
pub mod login_codes;
pub mod models;
pub mod permissions;
pub mod preferences;
pub mod sessions;
pub mod users;
pub mod wallet_nonces;

pub use models::{LoginType, Role, Theme, User, UserStatus};
