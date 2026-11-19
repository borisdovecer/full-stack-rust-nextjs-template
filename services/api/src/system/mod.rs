//! `/api/health` and `/api/config`: whether the API is up, and what the web needs before
//! anyone signs in.

mod dto;
mod handlers;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::health))
        .routes(routes!(handlers::public_config))
}
