//! `/api/auth/oauth`: "Continue with Google / GitHub".

mod cookies;
mod dto;
mod handlers;
mod providers;
pub mod repository;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

pub use cookies::{signup_removal, take_signup};
pub use dto::Intent;
pub use providers::{Provider, enabled};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::start))
        .routes(routes!(handlers::callback))
        .routes(routes!(handlers::pending_signup))
}
