//! `/api/help`: the system docs (`docs/system/*.md`) served to the Help page.

mod dto;
mod handlers;
mod service;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

pub use service::HelpDocs;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::help_index))
        .routes(routes!(handlers::help_page))
}
