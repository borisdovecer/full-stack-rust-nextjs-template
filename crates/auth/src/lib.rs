//! Authentication building blocks with no I/O: passwords, sign-in codes, wallet signatures,
//! session tokens, permissions.

pub mod code;
pub mod password;
pub mod permission;
pub mod pkce;
pub mod siws;
pub mod token;

pub use permission::{Permission, RequiredPermission};
