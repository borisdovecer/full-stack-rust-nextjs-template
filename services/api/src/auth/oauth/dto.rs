//! What the OAuth routes send and receive.

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    /// Sign in to an `external` account.
    Login,
    /// Start an account; the web's form finishes it.
    Register,
    /// Switch the signed-in account to this provider.
    Link,
}

impl Intent {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Intent::Login => "login",
            Intent::Register => "register",
            Intent::Link => "link",
        }
    }

    pub(super) fn parse(s: &str) -> Option<Self> {
        match s {
            "login" => Some(Intent::Login),
            "register" => Some(Intent::Register),
            "link" => Some(Intent::Link),
            _ => None,
        }
    }

    /// The page that shows how it went.
    pub(super) fn page(self) -> &'static str {
        match self {
            Intent::Login => "/login",
            Intent::Register => "/register",
            Intent::Link => "/settings",
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct StartParams {
    /// `login` (default), `register` or `link`.
    pub intent: Option<Intent>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    /// Set by the provider when the person cancelled or it failed.
    pub error: Option<String>,
}

/// What the registration form shows after the provider: the email and the names it gave.
#[derive(Debug, Serialize, ToSchema)]
pub struct PendingSignupDto {
    pub provider: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}
