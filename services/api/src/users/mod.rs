//! `/api/users`: user management for people with `users.read` / `users.manage`.

mod dto;
mod handlers;
mod service;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::list_users, handlers::create_user))
        .routes(routes!(handlers::get_user, handlers::update_user))
        .routes(routes!(handlers::set_password))
        .routes(routes!(handlers::set_login_method))
        .routes(routes!(handlers::set_permissions))
}
