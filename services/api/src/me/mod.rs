//! `/api/me`: the signed-in user's own account: profile, password, sign-in method, sessions,
//! preferences.

mod dto;
mod handlers;

use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::get_me, handlers::update_me))
        .routes(routes!(handlers::change_password))
        .routes(routes!(handlers::change_login_method))
        .routes(routes!(handlers::request_login_code))
        .routes(routes!(handlers::list_sessions))
        .routes(routes!(handlers::delete_session))
        .routes(routes!(handlers::update_preferences))
}
