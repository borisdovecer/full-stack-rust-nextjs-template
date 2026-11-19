//! "Continue with Google / GitHub": the round trip through the provider.
//!
//! `start` sends the browser to the provider with a random `state` and a PKCE challenge, both
//! kept in a short httpOnly cookie. `callback` is a GET that can open a session, so it is tied
//! to the browser that started it by that cookie: a `state` that does not match is refused.
//! Every outcome is a redirect to a web page, with `?error=<code>` when it failed.

use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use chrono::{Duration, Utc};
use serde_json::json;
use subtle::ConstantTimeEq;

use app_auth::{pkce, token};
use app_core::config::RegistrationMode;
use app_core::error::ErrorBody;
use app_core::extract::{Json, Path, Query};
use app_core::{AppResult, ErrorCode};

use super::cookies::{flow_cookie, flow_cookie_name, signup_cookie, signup_cookie_name};
use super::dto::{CallbackParams, Intent, PendingSignupDto, StartParams};
use super::providers::{Provider, ProviderAccount};
use super::repository::{self as oauth_signups, PendingSignup};
use crate::auth::repository::users::{self, LoginMethod};
use crate::auth::repository::{LoginType, UserStatus, identities, sessions};
use crate::auth::{
    self, Authenticated, Client, append_cookie, read_cookie, require_recent_sign_in,
};
use crate::shared::repository::audit::{self, action};
use crate::state::AppState;

const FLOW_MINUTES: i64 = 10;
const SIGNUP_MINUTES: i64 = 30;

/// Sends the browser to the provider. `404` when the provider is not configured.
#[utoipa::path(
    get,
    path = "/api/auth/oauth/{provider}/start",
    tag = "auth",
    params(("provider" = String, Path, description = "`google` or `github`"), StartParams),
    responses(
        (status = SEE_OTHER, description = "To the provider; sets the flow cookie"),
        (status = NOT_FOUND, description = "`not_found`: no such provider, or it is off", body = ErrorBody),
    )
)]
pub async fn start(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(params): Query<StartParams>,
    session: Option<Extension<Authenticated>>,
) -> AppResult<Response> {
    let provider = Provider::parse(&provider)
        .filter(|p| p.enabled(&state.config))
        .ok_or(ErrorCode::NotFound)?;
    let intent = params.intent.unwrap_or(Intent::Login);
    if intent == Intent::Link {
        // Fail before the trip to the provider rather than after it.
        if let Err(code) = check_link_session(&state, session.as_deref()) {
            return Ok(to_page(intent, Some(code)));
        }
    }
    let flow_state = pkce::state();
    let (verifier, challenge) = pkce::pair();
    let value = format!(
        "{}.{}.{flow_state}.{verifier}",
        provider.as_str(),
        intent.as_str()
    );
    let mut res = Redirect::to(&provider.authorize_url(&state.config, &flow_state, &challenge))
        .into_response();
    append_cookie(
        &mut res,
        flow_cookie(&state, value, Duration::minutes(FLOW_MINUTES)),
    );
    Ok(res)
}

/// Where the provider sends the browser back. Always a redirect to a web page.
#[utoipa::path(
    get,
    path = "/api/auth/oauth/{provider}/callback",
    tag = "auth",
    params(("provider" = String, Path, description = "`google` or `github`"), CallbackParams),
    responses((status = SEE_OTHER, description = "To `/`, `/register?via=external`, `/settings`, or the page with `?error=<code>`"))
)]
pub async fn callback(
    State(state): State<AppState>,
    client: Client,
    headers: HeaderMap,
    Path(provider): Path<String>,
    Query(params): Query<CallbackParams>,
    session: Option<Extension<Authenticated>>,
) -> Response {
    let mut res = handle_callback(
        &state,
        &client,
        &headers,
        &provider,
        params,
        session.as_deref(),
    )
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "oauth callback failed");
        to_page(Intent::Login, Some(ErrorCode::ProviderFailed))
    });
    let mut gone = flow_cookie(&state, String::new(), Duration::zero());
    gone.make_removal();
    append_cookie(&mut res, gone);
    res
}

async fn handle_callback(
    state: &AppState,
    client: &Client,
    headers: &HeaderMap,
    provider: &str,
    params: CallbackParams,
    session: Option<&Authenticated>,
) -> anyhow::Result<Response> {
    let cookie = read_cookie(headers, &flow_cookie_name(state)).unwrap_or_default();
    let mut parts = cookie.splitn(4, '.');
    let (Some(c_provider), Some(c_intent), Some(c_state), Some(verifier)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        tracing::info!("oauth callback without a flow cookie");
        return Ok(to_page(Intent::Login, Some(ErrorCode::ProviderFailed)));
    };
    let intent = Intent::parse(c_intent).unwrap_or(Intent::Login);
    let Some(provider) =
        Provider::parse(provider).filter(|p| p.as_str() == c_provider && p.enabled(&state.config))
    else {
        return Ok(to_page(intent, Some(ErrorCode::ProviderFailed)));
    };
    if params.error.as_deref() == Some("access_denied") {
        // The person cancelled at the provider: back to where they were, nothing to say.
        return Ok(to_page(intent, None));
    }
    let (Some(code), Some(given_state)) = (params.code, params.state) else {
        return Ok(to_page(intent, Some(ErrorCode::ProviderFailed)));
    };
    if !bool::from(given_state.as_bytes().ct_eq(c_state.as_bytes())) {
        tracing::warn!(provider = provider.as_str(), "oauth state mismatch");
        return Ok(to_page(intent, Some(ErrorCode::ProviderFailed)));
    }
    let account = match provider.account(state, &code, verifier).await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(provider = provider.as_str(), error = %e, "oauth exchange failed");
            return Ok(to_page(intent, Some(ErrorCode::ProviderFailed)));
        }
    };
    if !account.email_verified {
        return Ok(to_page(intent, Some(ErrorCode::EmailNotVerified)));
    }
    match intent {
        Intent::Login => login(state, client, provider, &account).await,
        Intent::Register => register(state, provider, account).await,
        Intent::Link => link(state, session, provider, &account).await,
    }
}

async fn login(
    state: &AppState,
    client: &Client,
    provider: Provider,
    account: &ProviderAccount,
) -> anyhow::Result<Response> {
    let p = provider.as_str();
    let mut user = identities::find_user(&state.db, p, &account.subject).await?;
    if user.is_none() {
        // An `external` account with no provider account tied yet (set up by hand): tie it by
        // the verified email.
        let by_email = users::find_by_email(&state.db, &account.email).await?;
        if let Some(u) = by_email.filter(|u| u.login_type == LoginType::External)
            && identities::provider_of(&state.db, u.id).await?.is_none()
        {
            identities::insert(&state.db, u.id, p, &account.subject, &account.email).await?;
            user = Some(u);
        }
    }
    let Some(user) = user.filter(|u| u.login_type == LoginType::External) else {
        return Ok(to_page(Intent::Login, Some(ErrorCode::InvalidCredentials)));
    };
    if user.status == UserStatus::Suspended {
        return Ok(to_page(Intent::Login, Some(ErrorCode::AccountSuspended)));
    }
    users::touch_last_login(&state.db, user.id).await?;
    let cookie = auth::start_session(state, user.id, client)
        .await
        .map_err(|e| anyhow::anyhow!("cannot start a session: {e:?}"))?;
    let mut res = Redirect::to("/").into_response();
    append_cookie(&mut res, cookie);
    Ok(res)
}

async fn register(
    state: &AppState,
    provider: Provider,
    account: ProviderAccount,
) -> anyhow::Result<Response> {
    if state.config.settings.registration.mode == RegistrationMode::Closed {
        return Ok(to_page(
            Intent::Register,
            Some(ErrorCode::RegistrationClosed),
        ));
    }
    let p = provider.as_str();
    let tied = identities::find_user(&state.db, p, &account.subject).await?;
    let taken = users::find_by_email(&state.db, &account.email).await?;
    if tied.is_some() || taken.is_some() {
        return Ok(to_page(Intent::Register, Some(ErrorCode::EmailTaken)));
    }
    let (token, hash) = token::generate();
    let signup = PendingSignup {
        provider: p.to_string(),
        subject: account.subject,
        email: account.email,
        first_name: account.first_name,
        last_name: account.last_name,
    };
    let expires_at = Utc::now() + Duration::minutes(SIGNUP_MINUTES);
    oauth_signups::create(&state.db, &hash, &signup, expires_at).await?;
    let mut res = Redirect::to("/register?via=external").into_response();
    append_cookie(
        &mut res,
        signup_cookie(state, token, Duration::minutes(SIGNUP_MINUTES)),
    );
    Ok(res)
}

async fn link(
    state: &AppState,
    session: Option<&Authenticated>,
    provider: Provider,
    account: &ProviderAccount,
) -> anyhow::Result<Response> {
    let auth = match check_link_session(state, session) {
        Ok(a) => a,
        Err(code) => return Ok(to_page(Intent::Link, Some(code))),
    };
    if account.email != auth.user.email {
        return Ok(to_page(
            Intent::Link,
            Some(ErrorCode::ProviderEmailMismatch),
        ));
    }
    let p = provider.as_str();
    if identities::find_user(&state.db, p, &account.subject)
        .await?
        .is_some()
    {
        return Ok(to_page(Intent::Link, Some(ErrorCode::IdentityTaken)));
    }
    let mut tx = state.db.begin().await?;
    let Some(user) = users::set_login_method(&mut *tx, auth.user.id, LoginMethod::External).await?
    else {
        return Ok(to_page(Intent::Link, Some(ErrorCode::Unauthenticated)));
    };
    identities::insert(&mut *tx, user.id, p, &account.subject, &account.email).await?;
    let revoked = sessions::revoke_all(&mut *tx, user.id, Some(auth.session_id)).await?;
    audit::record(
        &mut *tx,
        Some(user.id),
        Some(user.id),
        action::USER_LOGIN_METHOD_CHANGED,
        json!({
            "login_type": { "before": auth.user.login_type, "after": LoginType::External },
            "provider": p,
            "sessions_revoked": revoked,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(Redirect::to("/settings?changed=external").into_response())
}

/// The signed-in, active user who may switch to a provider now, or why not.
fn check_link_session<'a>(
    state: &AppState,
    session: Option<&'a Authenticated>,
) -> Result<&'a Authenticated, ErrorCode> {
    let auth = session.ok_or(ErrorCode::Unauthenticated)?;
    if auth.user.status != UserStatus::Active {
        return Err(ErrorCode::Forbidden);
    }
    if auth.user.login_type == LoginType::External {
        return Err(ErrorCode::LoginMethodUnchanged);
    }
    require_recent_sign_in(state, auth)?;
    Ok(auth)
}

/// The provider's answer waiting for the registration form, from the sign-up cookie.
#[utoipa::path(
    get,
    path = "/api/auth/oauth/signup",
    tag = "auth",
    responses(
        (status = OK, body = PendingSignupDto),
        (status = NOT_FOUND, description = "`not_found`: none, or it expired", body = ErrorBody),
    )
)]
pub async fn pending_signup(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<PendingSignupDto>> {
    let token = read_cookie(&headers, &signup_cookie_name(&state)).ok_or(ErrorCode::NotFound)?;
    let s = oauth_signups::find(&state.db, &token::hash(&token))
        .await?
        .ok_or(ErrorCode::NotFound)?;
    Ok(Json(PendingSignupDto {
        provider: s.provider,
        email: s.email,
        first_name: s.first_name,
        last_name: s.last_name,
    }))
}

fn to_page(intent: Intent, error: Option<ErrorCode>) -> Response {
    let page = intent.page();
    match error {
        Some(code) => {
            let code = serde_json::to_value(code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "provider_failed".into());
            Redirect::to(&format!("{page}?error={code}")).into_response()
        }
        None => Redirect::to(page).into_response(),
    }
}
