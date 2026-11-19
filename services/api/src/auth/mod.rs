//! `/api/auth`: register, sign in, sign out, and the sessions every other scope relies on.
//!
//! Other scopes use what this file re-exports: the session extractors (`SessionUser`,
//! `CurrentUser`, `Require<P>`), the session helpers, `Me`, the proof checks, and the sign-in
//! services.

pub mod conflicts;
mod dto;
mod handlers;
pub mod oauth;
pub mod repository;
mod service;
mod session;
mod validators;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

pub use dto::{Me, PreferencesDto, UserDto};
pub use repository::{LoginType, Role, Theme, User, UserStatus};
pub use service::passwords::Passwords;
pub use service::{codes, login_methods, throttle, wallet};
pub use session::*;
pub use validators::{MethodProof, ProofFields};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::register))
        .routes(routes!(handlers::login))
        .routes(routes!(handlers::request_code))
        .routes(routes!(handlers::wallet_nonce))
        .routes(routes!(handlers::logout))
        .routes(routes!(handlers::logout_all))
        .merge(oauth::routes())
}
