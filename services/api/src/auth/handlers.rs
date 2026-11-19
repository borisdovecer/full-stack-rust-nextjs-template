//! Register, log in, log out.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use app_core::config::RegistrationMode;
use app_core::error::ErrorBody;
use app_core::extract::Json;
use app_core::{AppError, AppResult, ErrorCode};

use super::conflicts;
use super::dto::{CodeRequest, LoginRequest, Me, RegisterRequest, WalletNonceRequest};
use super::oauth;
use super::repository::login_codes::CodePurpose;
use super::repository::users::{self, NewUser};
use super::repository::{LoginType, Role, UserStatus, identities, sessions};
use super::service::wallet::{self, Proof, WalletChallenge};
use super::service::{codes, login_methods, throttle};
use super::validators::ProofFields;
use crate::auth::{self, Client, SessionUser, load_me, signed_out};
use crate::shared::repository::audit::{self, action};
use crate::shared::validation::FieldErrors;
use crate::state::AppState;

/// Creates an account and signs it in. For `external`, the email and the provider account come
/// from the sign-up cookie that `GET /api/auth/oauth/{provider}/callback` set, never the body. With registration mode `approval` the account is
/// `pending` until an admin activates it.
#[utoipa::path(
    post,
    path = "/api/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = CREATED, description = "Registered and signed in; sets the session cookie", body = Me),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed`", body = ErrorBody),
        (status = UNPROCESSABLE_ENTITY, description = "`login_method_unavailable`", body = ErrorBody),
        (status = UNAUTHORIZED, description = "`invalid_code` or `invalid_signature`", body = ErrorBody),
        (status = TOO_MANY_REQUESTS, description = "`too_many_attempts`, `details.retry_at`", body = ErrorBody),
        (status = FORBIDDEN, description = "`registration_closed`", body = ErrorBody),
        (status = CONFLICT, description = "`email_taken` or `wallet_taken`", body = ErrorBody),
    )
)]
pub async fn register(
    State(state): State<AppState>,
    client: Client,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> AppResult<Response> {
    let status = match state.config.settings.registration.mode {
        RegistrationMode::Closed => return Err(ErrorCode::RegistrationClosed.into()),
        RegistrationMode::Open => UserStatus::Active,
        RegistrationMode::Approval => UserStatus::Pending,
    };
    let method = body.method.unwrap_or(LoginType::Password);
    login_methods::require(&state, method)?;

    let mut fields = FieldErrors::default();
    let email = fields.email("email", &body.email);
    let first_name = fields.name("first_name", &body.first_name);
    let last_name = fields.name("last_name", &body.last_name);
    let bio = body.bio.as_deref().and_then(|b| fields.bio("bio", b));
    // For `external`, the provider's answer is read below, from the sign-up cookie.
    let proof = fields.method_proof(
        method,
        ProofFields {
            password: body.password,
            login_code: body.login_code,
            address: body.address,
            message: body.message,
            signature: body.signature,
        },
    );
    fields.check()?;
    let signup = match method {
        LoginType::External => Some(
            oauth::take_signup(&state, &headers)
                .await?
                .ok_or(ErrorCode::ProviderFailed)?,
        ),
        _ => None,
    };
    let email = signup.as_ref().map_or(email, |s| s.email.clone());
    if let Some(given) = &proof.login_code {
        let ip = client.ip.as_deref();
        if !codes::spend(&state, &email, CodePurpose::Register, given, ip).await? {
            return Err(ErrorCode::InvalidCode.into());
        }
    }
    let address = match &proof.wallet {
        Some((address, message, signature)) => {
            let proof = Proof {
                address,
                message,
                signature,
            };
            if !wallet::verify(&state, proof).await? {
                return Err(ErrorCode::InvalidSignature.into());
            }
            Some(address.as_str())
        }
        None => None,
    };

    let hash = state.passwords.hash_some(proof.password).await?;

    let mut tx = state.db.begin().await?;
    let user = users::create(
        &mut *tx,
        NewUser {
            email: &email,
            first_name: &first_name,
            last_name: &last_name,
            bio: bio.as_deref(),
            login_type: method,
            password_hash: hash.as_deref(),
            wallet: address,
            role: Role::User,
            status,
        },
    )
    .await
    .map_err(conflicts::map)?;
    let mut data = json!({ "status": status, "login_type": method });
    if let Some(s) = &signup {
        identities::insert(&mut *tx, user.id, &s.provider, &s.subject, &s.email)
            .await
            .map_err(conflicts::map)?;
        data["provider"] = json!(s.provider);
    }
    audit::record(
        &mut *tx,
        Some(user.id),
        Some(user.id),
        action::USER_REGISTERED,
        data,
    )
    .await?;
    tx.commit().await?;

    let cookie = auth::start_session(&state, user.id, &client).await?;
    let me = load_me(&state, user).await?;
    let mut res = (StatusCode::CREATED, Json(me)).into_response();
    auth::append_cookie(&mut res, cookie);
    if signup.is_some() {
        auth::append_cookie(&mut res, oauth::signup_removal(&state));
    }
    Ok(res)
}

/// Signs in. An unknown email, an account that uses another method and a wrong proof all
/// answer the same `invalid_credentials`, so the answer never says how an email signs in.
#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = OK, description = "Signed in; sets the session cookie", body = Me),
        (status = UNAUTHORIZED, description = "`invalid_credentials`, or `invalid_code` for `code`", body = ErrorBody),
        (status = UNPROCESSABLE_ENTITY, description = "`login_method_unavailable`", body = ErrorBody),
        (status = FORBIDDEN, description = "`account_suspended`", body = ErrorBody),
        (status = TOO_MANY_REQUESTS, description = "`too_many_attempts`, `details.retry_at`", body = ErrorBody),
    )
)]
pub async fn login(
    State(state): State<AppState>,
    client: Client,
    Json(body): Json<LoginRequest>,
) -> AppResult<Response> {
    let method = body.method.unwrap_or(LoginType::Password);
    login_methods::require(&state, method)?;
    let email = crate::shared::validation::normalize_email(&body.email);

    if method == LoginType::Code {
        let given = body.login_code.unwrap_or_default();
        let ip = client.ip.as_deref();
        let spent = codes::spend(&state, &email, CodePurpose::Login, &given, ip).await?;
        let user = match users::find_by_email(&state.db, &email).await? {
            Some(u) if spent && u.login_type == LoginType::Code => u,
            _ => return Err(ErrorCode::InvalidCode.into()),
        };
        return finish_login(&state, &client, user).await;
    }

    if method == LoginType::Wallet {
        let attempt = throttle::begin(&state, &email, method, client.ip.as_deref()).await?;
        let address = body.address.as_deref().and_then(wallet::normalize_address);
        let ok = match (&address, body.message.as_deref(), body.signature.as_deref()) {
            (Some(address), Some(message), Some(signature)) => {
                let proof = Proof {
                    address,
                    message,
                    signature,
                };
                wallet::verify(&state, proof).await?
            }
            _ => false,
        };
        let user = users::find_by_email(&state.db, &email)
            .await?
            .filter(|u| ok && u.login_type == LoginType::Wallet && u.wallet == address);
        let Some(user) = user else {
            return Err(ErrorCode::InvalidCredentials.into());
        };
        attempt.succeeded(&state).await?;
        return finish_login(&state, &client, user).await;
    }

    if method != LoginType::Password {
        return Err(ErrorCode::LoginMethodUnavailable.into());
    }
    let attempt = throttle::begin(&state, &email, method, client.ip.as_deref()).await?;
    let creds = users::find_credentials_by_email(&state.db, &email).await?;
    // Only a password account's hash counts; any other case still spends the same time.
    let hash = creds
        .as_ref()
        .filter(|c| c.user.login_type == LoginType::Password)
        .and_then(|c| c.password_hash.clone());
    let password = body.password.unwrap_or_default();
    let ok = match hash {
        Some(hash) => state.passwords.verify(hash, password).await?,
        None => {
            state.passwords.verify_dummy(password).await?;
            false
        }
    };
    let Some(creds) = creds.filter(|_| ok) else {
        return Err(ErrorCode::InvalidCredentials.into());
    };
    attempt.succeeded(&state).await?;
    finish_login(&state, &client, creds.user).await
}

/// After the proof passed: refuses a suspended account, otherwise opens a session.
async fn finish_login(
    state: &AppState,
    client: &Client,
    user: super::repository::User,
) -> AppResult<Response> {
    if user.status == UserStatus::Suspended {
        return Err(ErrorCode::AccountSuspended.into());
    }
    users::touch_last_login(&state.db, user.id).await?;
    let cookie = auth::start_session(state, user.id, client).await?;
    let me = load_me(state, user).await?;
    let mut res = Json(me).into_response();
    auth::append_cookie(&mut res, cookie);
    Ok(res)
}

/// A challenge for a Solana wallet to sign, valid `wallet.nonce_minutes` and once. Answers the
/// same for any address: it says nothing about accounts.
#[utoipa::path(
    post,
    path = "/api/auth/wallet/nonce",
    tag = "auth",
    request_body = WalletNonceRequest,
    responses(
        (status = OK, body = WalletChallenge),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed` (`invalid_address`) or `login_method_unavailable`", body = ErrorBody),
        (status = TOO_MANY_REQUESTS, description = "`too_many_attempts`: this IP asked for too many", body = ErrorBody),
    )
)]
pub async fn wallet_nonce(
    State(state): State<AppState>,
    client: Client,
    Json(body): Json<WalletNonceRequest>,
) -> AppResult<Json<WalletChallenge>> {
    login_methods::require(&state, LoginType::Wallet)?;
    let address = wallet::normalize_address(&body.address)
        .ok_or_else(|| AppError::field("address", "invalid_address"))?;
    let challenge = wallet::challenge(&state, &address, client.ip.as_deref()).await?;
    Ok(Json(challenge))
}

/// Mails a 6-digit sign-in code. Always `204`, whether or not the address has an account, so the
/// answer never says who is a user. A code goes out only to an account that signs in with codes
/// (`login`), or to anyone registering (`register`). At most one mail per address per
/// `login_code.resend_seconds`.
#[utoipa::path(
    post,
    path = "/api/auth/code",
    tag = "auth",
    request_body = CodeRequest,
    responses(
        (status = NO_CONTENT, description = "Sent, if there is anyone to send it to"),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed` or `login_method_unavailable`", body = ErrorBody),
        (status = FORBIDDEN, description = "`registration_closed`", body = ErrorBody),
        (status = TOO_MANY_REQUESTS, description = "`too_many_attempts`: this IP asked for too many codes", body = ErrorBody),
    )
)]
pub async fn request_code(
    State(state): State<AppState>,
    client: Client,
    Json(body): Json<CodeRequest>,
) -> AppResult<StatusCode> {
    login_methods::require(&state, LoginType::Code)?;
    let purpose = body.purpose.unwrap_or(CodePurpose::Login);
    let mut fields = FieldErrors::default();
    let email = fields.email("email", &body.email);
    if purpose == CodePurpose::Change {
        fields.add("purpose", "invalid");
    }
    fields.check()?;

    let send = match purpose {
        CodePurpose::Register => {
            if state.config.settings.registration.mode == RegistrationMode::Closed {
                return Err(ErrorCode::RegistrationClosed.into());
            }
            true
        }
        _ => users::find_by_email(&state.db, &email)
            .await?
            .is_some_and(|u| u.login_type == LoginType::Code && u.status != UserStatus::Suspended),
    };
    codes::issue(&state, &email, purpose, client.ip.as_deref(), send).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ends this session.
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "auth",
    responses(
        (status = NO_CONTENT, description = "Signed out; clears the session cookie"),
        (status = UNAUTHORIZED, description = "`unauthenticated`", body = ErrorBody),
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    SessionUser(auth_): SessionUser,
) -> AppResult<Response> {
    sessions::revoke(&state.db, auth_.session_id, auth_.user.id).await?;
    Ok(signed_out(&state))
}

/// Ends every session of the signed-in user, on every device.
#[utoipa::path(
    post,
    path = "/api/auth/logout-all",
    tag = "auth",
    responses(
        (status = NO_CONTENT, description = "Signed out everywhere; clears the session cookie"),
        (status = UNAUTHORIZED, description = "`unauthenticated`", body = ErrorBody),
    )
)]
pub async fn logout_all(
    State(state): State<AppState>,
    SessionUser(auth_): SessionUser,
) -> AppResult<Response> {
    sessions::revoke_all(&state.db, auth_.user.id, None).await?;
    Ok(signed_out(&state))
}
