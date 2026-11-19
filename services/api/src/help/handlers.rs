//! The help index and its pages.

use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;

use app_core::error::ErrorBody;
use app_core::extract::{Json, Path};
use app_core::{AppResult, ErrorCode};

use super::dto::HelpEntry;
use crate::auth::SessionUser;
use crate::state::AppState;

/// The help pages, in the order the index lists them.
#[utoipa::path(
    get,
    path = "/api/help",
    tag = "help",
    responses(
        (status = OK, body = Vec<HelpEntry>),
        (status = UNAUTHORIZED, description = "`unauthenticated`", body = ErrorBody),
    )
)]
pub async fn help_index(State(state): State<AppState>, _: SessionUser) -> Json<Vec<HelpEntry>> {
    Json(state.help.index())
}

/// One help page as markdown.
#[utoipa::path(
    get,
    path = "/api/help/{slug}",
    tag = "help",
    params(("slug" = String, Path, description = "From the index, for example `architecture`")),
    responses(
        (status = OK, content_type = "text/markdown", body = String),
        (status = NOT_FOUND, description = "`not_found`", body = ErrorBody),
    )
)]
pub async fn help_page(
    State(state): State<AppState>,
    _: SessionUser,
    Path(slug): Path<String>,
) -> AppResult<impl IntoResponse> {
    let page = state.help.page(&slug).ok_or(ErrorCode::NotFound)?;
    Ok((
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        page,
    ))
}
