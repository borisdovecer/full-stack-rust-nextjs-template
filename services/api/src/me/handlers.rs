//! The signed-in user's own account: profile, password, sign-in method, sessions, preferences.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use uuid::Uuid;

use app_core::error::ErrorBody;
use app_core::extract::{Json, Path};
use app_core::{AppError, AppResult, ErrorCode};

use super::dto::{
    ChangeLoginMethodRequest, ChangePasswordRequest, SessionDto, UpdateMeRequest,
    UpdatePreferencesRequest,
};
use crate::auth::conflicts;
use crate::auth::repository::login_codes::CodePurpose;
use crate::auth::repository::preferences::{self, PreferencesPatch};
use crate::auth::repository::users::{self, LoginMethod, UserPatch};
use crate::auth::repository::{LoginType, sessions};
use crate::auth::wallet::{self, Proof};
use crate::auth::{Client, CurrentUser, SessionUser, load_me, require_recent_sign_in, signed_out};
use crate::auth::{Me, ProofFields, codes, login_methods, throttle};
use crate::shared::repository::audit::{self, action};
use crate::shared::validation::FieldErrors;
use crate::state::AppState;

/// The signed-in user. Works for `pending` users too, so the web can show the waiting page.
#[utoipa::path(
    get,
    path = "/api/me",
    tag = "me",
    responses(
        (status = OK, body = Me),
        (status = UNAUTHORIZED, description = "`unauthenticated`", body = ErrorBody),
    )
)]
pub async fn get_me(
    State(state): State<AppState>,
    SessionUser(auth): SessionUser,
) -> AppResult<Json<Me>> {
    Ok(Json(load_me(&state, auth.user).await?))
}

/// Changes the signed-in user's name or bio. A field left out stays as it is.
#[utoipa::path(
    patch,
    path = "/api/me",
    tag = "me",
    request_body = UpdateMeRequest,
    responses(
        (status = OK, body = Me),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed`", body = ErrorBody),
    )
)]
pub async fn update_me(
    State(state): State<AppState>,
    CurrentUser(auth): CurrentUser,
    Json(body): Json<UpdateMeRequest>,
) -> AppResult<Json<Me>> {
    let mut fields = FieldErrors::default();
    let first_name = body
        .first_name
        .as_deref()
        .map(|n| fields.name("first_name", n));
    let last_name = body
        .last_name
        .as_deref()
        .map(|n| fields.name("last_name", n));
    let bio = body.bio.as_deref().map(|b| fields.bio("bio", b));
    fields.check()?;

    let user = users::update(
        &state.db,
        auth.user.id,
        UserPatch {
            first_name: first_name.as_deref(),
            last_name: last_name.as_deref(),
            bio: bio.as_ref().map(Option::as_deref),
            ..Default::default()
        },
    )
    .await?
    .ok_or(ErrorCode::Unauthenticated)?;
    Ok(Json(load_me(&state, user).await?))
}

/// Changes the password and signs out every other session. A wrong current password counts
/// against the same limits as a failed login, so a stolen session cannot guess it freely.
#[utoipa::path(
    post,
    path = "/api/me/password",
    tag = "me",
    request_body = ChangePasswordRequest,
    responses(
        (status = NO_CONTENT, description = "Changed"),
        (status = UNPROCESSABLE_ENTITY, description = "`wrong_password` or `validation_failed`", body = ErrorBody),
        (status = TOO_MANY_REQUESTS, description = "`too_many_attempts`, `details.retry_at`", body = ErrorBody),
    )
)]
pub async fn change_password(
    State(state): State<AppState>,
    client: Client,
    CurrentUser(auth): CurrentUser,
    Json(body): Json<ChangePasswordRequest>,
) -> AppResult<StatusCode> {
    let mut fields = FieldErrors::default();
    fields.new_password("new_password", &body.new_password);
    fields.check()?;

    let creds = users::find_credentials(&state.db, auth.user.id)
        .await?
        .ok_or(ErrorCode::Unauthenticated)?;
    let Some(current_hash) = creds.password_hash else {
        return Err(ErrorCode::WrongPassword.into());
    };
    let ip = client.ip.as_deref();
    let attempt = throttle::begin(&state, &auth.user.email, LoginType::Password, ip).await?;
    if !state
        .passwords
        .verify(current_hash, body.current_password)
        .await?
    {
        return Err(ErrorCode::WrongPassword.into());
    }
    attempt.succeeded(&state).await?;
    let hash = state.passwords.hash(body.new_password).await?;

    let mut tx = state.db.begin().await?;
    users::set_password(&mut *tx, auth.user.id, &hash).await?;
    let revoked = sessions::revoke_all(&mut *tx, auth.user.id, Some(auth.session_id)).await?;
    audit::record(
        &mut *tx,
        Some(auth.user.id),
        Some(auth.user.id),
        action::USER_PASSWORD_CHANGED,
        json!({ "sessions_revoked": revoked }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Mails a code to the signed-in user's own email, to prove the inbox before switching the
/// account to codes.
#[utoipa::path(
    post,
    path = "/api/me/login-code",
    tag = "me",
    responses(
        (status = NO_CONTENT, description = "Sent"),
        (status = UNPROCESSABLE_ENTITY, description = "`login_method_unavailable`", body = ErrorBody),
        (status = CONFLICT, description = "`login_method_unchanged`: the account already uses codes", body = ErrorBody),
        (status = FORBIDDEN, description = "`reauth_required`", body = ErrorBody),
        (status = TOO_MANY_REQUESTS, description = "`too_many_attempts`", body = ErrorBody),
    )
)]
pub async fn request_login_code(
    State(state): State<AppState>,
    client: Client,
    CurrentUser(auth): CurrentUser,
) -> AppResult<StatusCode> {
    login_methods::require(&state, LoginType::Code)?;
    if auth.user.login_type == LoginType::Code {
        return Err(ErrorCode::LoginMethodUnchanged.into());
    }
    require_recent_sign_in(&state, &auth)?;
    let ip = client.ip.as_deref();
    codes::issue(&state, &auth.user.email, CodePurpose::Change, ip, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Changes how the signed-in user signs in. Needs a session that signed in within
/// `session.reauth_minutes`, and proof of the new method. The old method's credential is cleared
/// and every other session ends.
#[utoipa::path(
    put,
    path = "/api/me/login-method",
    tag = "me",
    request_body = ChangeLoginMethodRequest,
    responses(
        (status = OK, body = Me),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed` or `login_method_unavailable`", body = ErrorBody),
        (status = CONFLICT, description = "`login_method_unchanged` (a password is changed at `/api/me/password`) or `wallet_taken`", body = ErrorBody),
        (status = FORBIDDEN, description = "`reauth_required`: the session signed in too long ago", body = ErrorBody),
        (status = UNAUTHORIZED, description = "`invalid_code` or `invalid_signature`", body = ErrorBody),
    )
)]
pub async fn change_login_method(
    State(state): State<AppState>,
    client: Client,
    CurrentUser(auth): CurrentUser,
    Json(body): Json<ChangeLoginMethodRequest>,
) -> AppResult<Json<Me>> {
    login_methods::require(&state, body.method)?;
    if auth.user.login_type == body.method {
        return Err(ErrorCode::LoginMethodUnchanged.into());
    }
    require_recent_sign_in(&state, &auth)?;
    // `external` is switched to through the provider (`GET /api/auth/oauth/{p}/start?intent=link`).
    if body.method == LoginType::External {
        return Err(ErrorCode::LoginMethodUnavailable.into());
    }
    let mut fields = FieldErrors::default();
    let proof = fields.method_proof(
        body.method,
        ProofFields {
            password: body.password,
            login_code: body.login_code,
            address: body.address,
            message: body.message,
            signature: body.signature,
        },
    );
    fields.check()?;

    let hash = state.passwords.hash_some(proof.password).await?;
    let method = match (body.method, hash.as_deref()) {
        (LoginType::Password, Some(hash)) => LoginMethod::Password { hash },
        (LoginType::Code, _) => {
            let given = proof.login_code.unwrap_or_default();
            let ip = client.ip.as_deref();
            let email = &auth.user.email;
            if !codes::spend(&state, email, CodePurpose::Change, &given, ip).await? {
                return Err(ErrorCode::InvalidCode.into());
            }
            LoginMethod::Code
        }
        (LoginType::Wallet, _) => {
            let Some((address, message, signature)) = &proof.wallet else {
                return Err(ErrorCode::LoginMethodUnavailable.into());
            };
            let proof = Proof {
                address,
                message,
                signature,
            };
            if !wallet::verify(&state, proof).await? {
                return Err(ErrorCode::InvalidSignature.into());
            }
            LoginMethod::Wallet { address }
        }
        _ => return Err(ErrorCode::LoginMethodUnavailable.into()),
    };

    let mut tx = state.db.begin().await?;
    let user = users::set_login_method(&mut *tx, auth.user.id, method)
        .await
        .map_err(conflicts::map)?
        .ok_or(ErrorCode::Unauthenticated)?;
    let revoked = sessions::revoke_all(&mut *tx, auth.user.id, Some(auth.session_id)).await?;
    audit::record(
        &mut *tx,
        Some(auth.user.id),
        Some(auth.user.id),
        action::USER_LOGIN_METHOD_CHANGED,
        json!({
            "login_type": { "before": auth.user.login_type, "after": user.login_type },
            "sessions_revoked": revoked,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(load_me(&state, user).await?))
}

/// The signed-in user's live sessions, most recently used first.
#[utoipa::path(
    get,
    path = "/api/me/sessions",
    tag = "me",
    responses((status = OK, body = Vec<SessionDto>))
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    CurrentUser(auth): CurrentUser,
) -> AppResult<Json<Vec<SessionDto>>> {
    let list = sessions::list_active(&state.db, auth.user.id).await?;
    Ok(Json(
        list.into_iter()
            .map(|s| SessionDto::new(s, auth.session_id))
            .collect(),
    ))
}

/// Ends one of the signed-in user's sessions. Ending the current one signs out.
#[utoipa::path(
    delete,
    path = "/api/me/sessions/{id}",
    tag = "me",
    params(("id" = Uuid, Path, description = "Session id")),
    responses(
        (status = NO_CONTENT, description = "Ended"),
        (status = NOT_FOUND, description = "`not_found`: not yours, or already ended", body = ErrorBody),
    )
)]
pub async fn delete_session(
    State(state): State<AppState>,
    CurrentUser(auth): CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Response> {
    if !sessions::revoke(&state.db, id, auth.user.id).await? {
        return Err(AppError::new(ErrorCode::NotFound));
    }
    if id == auth.session_id {
        return Ok(signed_out(&state));
    }
    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Changes the signed-in user's preferences.
#[utoipa::path(
    patch,
    path = "/api/me/preferences",
    tag = "me",
    request_body = UpdatePreferencesRequest,
    responses((status = OK, body = Me))
)]
pub async fn update_preferences(
    State(state): State<AppState>,
    SessionUser(auth): SessionUser,
    Json(body): Json<UpdatePreferencesRequest>,
) -> AppResult<Json<Me>> {
    preferences::update(
        &state.db,
        auth.user.id,
        PreferencesPatch { theme: body.theme },
    )
    .await?;
    Ok(Json(load_me(&state, auth.user).await?))
}
