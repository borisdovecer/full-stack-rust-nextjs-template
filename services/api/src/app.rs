//! Builds the router and the OpenAPI document from the same route list, so they cannot drift.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultOnResponse, TraceLayer};
use tracing::Level;
use utoipa::OpenApi;
use utoipa::openapi::OpenApi as OpenApiDoc;
use utoipa_axum::router::OpenApiRouter;

use app_core::error::{ErrorBody, ErrorCode};
use app_core::{AppError, request_id};

use crate::auth::{self, Require, UsersManage};
use crate::shared::middleware;
use crate::state::AppState;
use crate::{help, me, system, users};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const BODY_LIMIT_BYTES: usize = 64 * 1024;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "App API",
        description = "The HTTP API. Every error answers with `ErrorBody`; decide on `error.code`."
    ),
    // Schemas only referenced from query parameters are not collected automatically.
    components(schemas(ErrorBody, ErrorCode, crate::auth::repository::users::UserSort, auth::oauth::Intent)),
    tags(
        (name = "system", description = "Health and public settings"),
        (name = "auth", description = "Register, log in, log out"),
        (name = "me", description = "The signed-in user's own account"),
        (name = "help", description = "The system docs for the Help page"),
        (name = "users", description = "User management: needs `users.read` or `users.manage`"),
    )
)]
struct ApiDoc;

/// The routes and their OpenAPI description: one router per scope.
fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(system::routes())
        .merge(auth::routes())
        .merge(me::routes())
        .merge(help::routes())
        .merge(users::routes())
}

/// The OpenAPI document, without building a server.
pub fn openapi() -> OpenApiDoc {
    api_routes().split_for_parts().1
}

/// The full application with middleware.
pub fn router(state: AppState) -> Router {
    let (api, doc) = api_routes().split_for_parts();
    let spec = Arc::new(doc.to_pretty_json().unwrap_or_default());

    let production = state.config.env.is_production();

    let router = api
        .route(
            "/api/openapi.json",
            get(move |allowed: Result<Require<UsersManage>, AppError>| {
                let spec = Arc::clone(&spec);
                async move {
                    // In production the spec is for user managers; anyone may read it in
                    // development. Others get `404`, so it does not say it exists.
                    if production && allowed.is_err() {
                        return AppError::new(ErrorCode::NotFound).into_response();
                    }
                    (
                        [(header::CONTENT_TYPE, "application/json")],
                        (*spec).clone(),
                    )
                        .into_response()
                }
            }),
        )
        .fallback(|| async { AppError::new(ErrorCode::NotFound) })
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::session,
        ))
        .layer(axum::middleware::from_fn(middleware::require_json_writes))
        .with_state(state);

    // Layers run outside-in in the order they are added last-to-first: the request id wraps
    // everything, so every log line and error body inside carries it.
    router
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "request",
                        method = %req.method(),
                        path = %req.uri().path(),
                        request_id = request_id::current().as_deref().unwrap_or_default(),
                    )
                })
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(axum::middleware::from_fn(middleware::normalize_errors))
        .layer(CatchPanicLayer::new())
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn(middleware::request_id))
}
