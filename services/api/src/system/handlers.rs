//! Health and the public settings.

use axum::extract::State;
use axum::http::StatusCode;

use app_core::extract::Json;

use super::dto::{Health, PublicConfig};
use crate::auth::{login_methods, oauth};
use crate::state::AppState;
use crate::telemetry::{GIT_COMMIT, VERSION};

/// Whether the API and its database are up, and which build is running.
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "system",
    responses(
        (status = OK, body = Health),
        (status = SERVICE_UNAVAILABLE, description = "A check failed", body = Health),
    )
)]
pub async fn health(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    use app_db::MigrationState as M;
    let (database, migrations) = match app_db::migration_state(&state.db).await {
        Ok(M::Current) => ("ok", "current"),
        Ok(M::Pending(_)) => ("ok", "pending"),
        Ok(M::Failed(_)) => ("ok", "failed"),
        Ok(M::Changed(_)) => ("ok", "changed"),
        Ok(M::Unknown(_)) => ("ok", "unknown"),
        Err(e) => {
            tracing::warn!(error = %e, "health: database unreachable");
            ("down", "unchecked")
        }
    };
    let healthy = database == "ok" && migrations == "current";
    let status = if healthy {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (
        status,
        Json(Health {
            status: if healthy { "ok" } else { "degraded" },
            version: VERSION,
            commit: GIT_COMMIT,
            mail: state.mailer.mode(),
            environment: state.config.env,
            database,
            migrations,
        }),
    )
}

/// Public settings for the web: app name, registration mode, sign-in methods, version.
#[utoipa::path(
    get,
    path = "/api/config",
    tag = "system",
    responses((status = OK, body = PublicConfig))
)]
pub async fn public_config(State(state): State<AppState>) -> Json<PublicConfig> {
    let s = &state.config.settings;
    Json(PublicConfig {
        app_name: s.app.name.clone(),
        registration_mode: s.registration.mode,
        login_methods: login_methods::available(&state),
        admin_login_methods: login_methods::admin_assignable(&state),
        oauth_providers: oauth::enabled(&state.config)
            .into_iter()
            .map(|p| p.as_str())
            .collect(),
        version: VERSION,
    })
}
