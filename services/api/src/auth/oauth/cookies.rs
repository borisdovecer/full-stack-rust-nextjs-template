//! The two short cookies of the round trip: the flow cookie (`state` and the PKCE verifier,
//! for the callback) and the sign-up cookie (the provider's answer, for the registration form).

use axum::http::HeaderMap;
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::Duration;

use app_auth::token;
use app_core::AppResult;

use super::repository::{self as oauth_signups, PendingSignup};
use crate::auth::read_cookie;
use crate::state::AppState;

/// Takes the provider's waiting answer for `POST /api/auth/register` with `method: external`.
pub async fn take_signup(
    state: &AppState,
    headers: &HeaderMap,
) -> AppResult<Option<PendingSignup>> {
    let Some(token) = read_cookie(headers, &signup_cookie_name(state)) else {
        return Ok(None);
    };
    Ok(oauth_signups::take(&state.db, &token::hash(&token)).await?)
}

/// A cookie that deletes the sign-up cookie.
pub fn signup_removal(state: &AppState) -> Cookie<'static> {
    let mut c = signup_cookie(state, String::new(), Duration::zero());
    c.make_removal();
    c
}

pub(super) fn flow_cookie_name(state: &AppState) -> String {
    format!("{}_oauth", state.config.settings.session.cookie_name)
}

pub(super) fn signup_cookie_name(state: &AppState) -> String {
    format!("{}_signup", state.config.settings.session.cookie_name)
}

pub(super) fn flow_cookie(state: &AppState, value: String, age: Duration) -> Cookie<'static> {
    short_cookie(
        state,
        flow_cookie_name(state),
        value,
        "/api/auth/oauth",
        age,
    )
}

pub(super) fn signup_cookie(state: &AppState, value: String, age: Duration) -> Cookie<'static> {
    short_cookie(state, signup_cookie_name(state), value, "/api/auth", age)
}

fn short_cookie(
    state: &AppState,
    name: String,
    value: String,
    path: &'static str,
    age: Duration,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .http_only(true)
        .secure(state.config.env.is_production())
        .same_site(SameSite::Lax)
        .path(path)
        .max_age(cookie::time::Duration::seconds(age.num_seconds()))
        .build()
}
