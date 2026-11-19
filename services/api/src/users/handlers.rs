//! User management for people with `users.read` / `users.manage`.
//!
//! Rules every change keeps:
//! - Nobody changes their own role, status or permissions (`cannot_change_self`).
//! - Only an admin changes an admin's account or makes someone an admin (`forbidden`), so
//!   `users.manage` cannot be used to climb to admin.
//! - The last active admin cannot be demoted or suspended (`last_admin`).
//! - Every change writes an audit event in the same transaction.
//! - An admin gives an account only a method that needs no secret from the person
//!   ([`login_methods::admin_assignable`]): a password the admin sets, or an email code.

use axum::extract::State;
use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use app_auth::Permission;
use app_core::error::ErrorBody;
use app_core::extract::{Json, Path, Query};
use app_core::{AppResult, ErrorCode};

use super::dto::{
    CreateUserRequest, ListParams, SetLoginMethodRequest, SetPasswordRequest,
    SetPermissionsRequest, UpdateUserRequest, UserDetail, UserPage,
};
use super::service::{diff, is_admin, load_detail};
use crate::auth::UserDto;
use crate::auth::conflicts;
use crate::auth::repository::users::{self, ListUsers, LoginMethod, NewUser, UserPatch};
use crate::auth::repository::{LoginType, Role, UserStatus, permissions, sessions};
use crate::auth::{Require, UsersManage, UsersRead, login_methods};
use crate::shared::repository::audit::{self, action};
use crate::shared::validation::FieldErrors;
use crate::state::AppState;

/// A page of users.
#[utoipa::path(
    get,
    path = "/api/users",
    tag = "users",
    params(ListParams),
    responses(
        (status = OK, body = UserPage),
        (status = FORBIDDEN, description = "`forbidden`: needs `users.read`", body = ErrorBody),
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    _: Require<UsersRead>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<UserPage>> {
    let page = p.page.unwrap_or(1).max(1);
    let per_page = p.per_page.unwrap_or(20).clamp(1, 100);
    let mut conn = state.db.acquire().await?;
    let result = users::list(
        &mut conn,
        &ListUsers {
            search: p.search,
            role: p.role,
            status: p.status,
            sort: p.sort.unwrap_or_default(),
            page,
            per_page,
        },
    )
    .await?;
    Ok(Json(UserPage {
        items: result.items.into_iter().map(UserDto::from).collect(),
        total: result.total,
        total_capped: result.capped,
        page,
        per_page,
    }))
}

/// One user in detail.
#[utoipa::path(
    get,
    path = "/api/users/{id}",
    tag = "users",
    params(("id" = Uuid, Path)),
    responses(
        (status = OK, body = UserDetail),
        (status = NOT_FOUND, description = "`not_found`", body = ErrorBody),
    )
)]
pub async fn get_user(
    State(state): State<AppState>,
    _: Require<UsersRead>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserDetail>> {
    let user = users::find(&state.db, id)
        .await?
        .ok_or(ErrorCode::NotFound)?;
    Ok(Json(load_detail(&state, user).await?))
}

/// Creates a user. The only way in when registration is `closed`.
#[utoipa::path(
    post,
    path = "/api/users",
    tag = "users",
    request_body = CreateUserRequest,
    responses(
        (status = CREATED, body = UserDetail),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed`", body = ErrorBody),
        (status = UNPROCESSABLE_ENTITY, description = "`login_method_unavailable`", body = ErrorBody),
        (status = CONFLICT, description = "`email_taken`", body = ErrorBody),
        (status = FORBIDDEN, description = "`forbidden`: only an admin creates an admin", body = ErrorBody),
    )
)]
pub async fn create_user(
    State(state): State<AppState>,
    Require(actor, _): Require<UsersManage>,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<(StatusCode, Json<UserDetail>)> {
    let role = body.role.unwrap_or(Role::User);
    let status = body.status.unwrap_or(UserStatus::Active);
    if role == Role::Admin && !is_admin(&actor) {
        return Err(ErrorCode::Forbidden.into());
    }
    let method = body.method.unwrap_or(LoginType::Password);
    login_methods::require_admin_assignable(&state, method)?;

    let mut fields = FieldErrors::default();
    let email = fields.email("email", &body.email);
    let first_name = fields.name("first_name", &body.first_name);
    let last_name = fields.name("last_name", &body.last_name);
    let bio = body.bio.as_deref().and_then(|b| fields.bio("bio", b));
    let password = match method {
        LoginType::Password => fields.required_new_password("password", body.password),
        _ => None,
    };
    fields.check()?;
    let hash = state.passwords.hash_some(password).await?;

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
            wallet: None,
            role,
            status,
        },
    )
    .await
    .map_err(conflicts::map)?;
    audit::record(
        &mut *tx,
        Some(actor.user.id),
        Some(user.id),
        action::USER_CREATED,
        json!({ "role": role, "status": status, "login_type": method }),
    )
    .await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(load_detail(&state, user).await?)))
}

/// Changes a user's name, bio, role or status. Suspending signs them out everywhere.
#[utoipa::path(
    patch,
    path = "/api/users/{id}",
    tag = "users",
    params(("id" = Uuid, Path)),
    request_body = UpdateUserRequest,
    responses(
        (status = OK, body = UserDetail),
        (status = CONFLICT, description = "`cannot_change_self` or `last_admin`", body = ErrorBody),
        (status = FORBIDDEN, description = "`forbidden`: only an admin changes an admin", body = ErrorBody),
        (status = NOT_FOUND, description = "`not_found`", body = ErrorBody),
    )
)]
pub async fn update_user(
    State(state): State<AppState>,
    Require(actor, _): Require<UsersManage>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUserRequest>,
) -> AppResult<Json<UserDetail>> {
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

    let mut tx = state.db.begin().await?;
    // Lock admins first, in a fixed order, so two concurrent demotions cannot both pass.
    let active_admins = users::lock_active_admins(&mut tx).await?;
    let before = users::find(&mut *tx, id)
        .await?
        .ok_or(ErrorCode::NotFound)?;

    let role = body.role.filter(|r| *r != before.role);
    let status = body.status.filter(|s| *s != before.status);
    if (role.is_some() || status.is_some()) && before.id == actor.user.id {
        return Err(ErrorCode::CannotChangeSelf.into());
    }
    let touches_admin = before.role == Role::Admin || role == Some(Role::Admin);
    if touches_admin && !is_admin(&actor) {
        return Err(ErrorCode::Forbidden.into());
    }
    let was_active_admin = before.role == Role::Admin && before.status == UserStatus::Active;
    let stays_active_admin = role.unwrap_or(before.role) == Role::Admin
        && status.unwrap_or(before.status) == UserStatus::Active;
    if was_active_admin && !stays_active_admin && active_admins.len() <= 1 {
        return Err(ErrorCode::LastAdmin.into());
    }

    let after = users::update(
        &mut *tx,
        id,
        UserPatch {
            first_name: first_name.as_deref(),
            last_name: last_name.as_deref(),
            bio: bio.as_ref().map(Option::as_deref),
            role,
            status,
        },
    )
    .await?
    .ok_or(ErrorCode::NotFound)?;

    let revoked = if status == Some(UserStatus::Suspended) {
        sessions::revoke_all(&mut *tx, id, None).await?
    } else {
        0
    };

    let changes = diff(&before, &after);
    if !changes.is_empty() {
        let mut data = json!({ "changes": changes });
        if revoked > 0 {
            data["sessions_revoked"] = json!(revoked);
        }
        audit::record(
            &mut *tx,
            Some(actor.user.id),
            Some(id),
            action::USER_UPDATED,
            data,
        )
        .await?;
    }
    tx.commit().await?;

    Ok(Json(load_detail(&state, after).await?))
}

/// Sets a new password for a user and signs them out everywhere. The way back in for someone
/// who forgot their password. An account that used another method signs in with this password
/// from now on. Not for your own account: that is `POST /api/me/password`.
#[utoipa::path(
    put,
    path = "/api/users/{id}/password",
    tag = "users",
    params(("id" = Uuid, Path)),
    request_body = SetPasswordRequest,
    responses(
        (status = NO_CONTENT, description = "Set"),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed`", body = ErrorBody),
        (status = CONFLICT, description = "`cannot_change_self`", body = ErrorBody),
        (status = FORBIDDEN, description = "`forbidden`: only an admin sets an admin's password", body = ErrorBody),
        (status = NOT_FOUND, description = "`not_found`", body = ErrorBody),
    )
)]
pub async fn set_password(
    State(state): State<AppState>,
    Require(actor, _): Require<UsersManage>,
    Path(id): Path<Uuid>,
    Json(body): Json<SetPasswordRequest>,
) -> AppResult<StatusCode> {
    let mut fields = FieldErrors::default();
    fields.new_password("password", &body.password);
    fields.check()?;

    if id == actor.user.id {
        return Err(ErrorCode::CannotChangeSelf.into());
    }
    let target = users::find(&state.db, id)
        .await?
        .ok_or(ErrorCode::NotFound)?;
    if target.role == Role::Admin && !is_admin(&actor) {
        return Err(ErrorCode::Forbidden.into());
    }
    let hash = state.passwords.hash(body.password).await?;

    let mut tx = state.db.begin().await?;
    users::set_login_method(&mut *tx, id, LoginMethod::Password { hash: &hash }).await?;
    let revoked = sessions::revoke_all(&mut *tx, id, None).await?;
    let mut data = json!({ "sessions_revoked": revoked });
    if target.login_type != LoginType::Password {
        data["login_type"] = json!({ "before": target.login_type, "after": LoginType::Password });
    }
    audit::record(
        &mut *tx,
        Some(actor.user.id),
        Some(id),
        action::USER_PASSWORD_SET,
        data,
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Changes how a user signs in, for someone who lost their wallet or provider account. Clears
/// the old method's credential and signs them out everywhere.
#[utoipa::path(
    put,
    path = "/api/users/{id}/login-method",
    tag = "users",
    params(("id" = Uuid, Path)),
    request_body = SetLoginMethodRequest,
    responses(
        (status = OK, body = UserDetail),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed` or `login_method_unavailable`", body = ErrorBody),
        (status = CONFLICT, description = "`cannot_change_self` or `login_method_unchanged`", body = ErrorBody),
        (status = FORBIDDEN, description = "`forbidden`: only an admin changes an admin", body = ErrorBody),
        (status = NOT_FOUND, description = "`not_found`", body = ErrorBody),
    )
)]
pub async fn set_login_method(
    State(state): State<AppState>,
    Require(actor, _): Require<UsersManage>,
    Path(id): Path<Uuid>,
    Json(body): Json<SetLoginMethodRequest>,
) -> AppResult<Json<UserDetail>> {
    login_methods::require_admin_assignable(&state, body.method)?;
    let mut fields = FieldErrors::default();
    let password = match body.method {
        LoginType::Password => fields.required_new_password("password", body.password),
        _ => None,
    };
    fields.check()?;

    if id == actor.user.id {
        return Err(ErrorCode::CannotChangeSelf.into());
    }
    let target = users::find(&state.db, id)
        .await?
        .ok_or(ErrorCode::NotFound)?;
    if target.role == Role::Admin && !is_admin(&actor) {
        return Err(ErrorCode::Forbidden.into());
    }
    // Choosing `password` again sets a new one; any other unchanged method would do nothing.
    if target.login_type == body.method && body.method != LoginType::Password {
        return Err(ErrorCode::LoginMethodUnchanged.into());
    }
    let hash = state.passwords.hash_some(password).await?;
    let method = match (body.method, hash.as_deref()) {
        (LoginType::Password, Some(hash)) => LoginMethod::Password { hash },
        (LoginType::Code, _) => LoginMethod::Code,
        _ => return Err(ErrorCode::LoginMethodUnavailable.into()),
    };

    let mut tx = state.db.begin().await?;
    let after = users::set_login_method(&mut *tx, id, method)
        .await?
        .ok_or(ErrorCode::NotFound)?;
    let revoked = sessions::revoke_all(&mut *tx, id, None).await?;
    audit::record(
        &mut *tx,
        Some(actor.user.id),
        Some(id),
        action::USER_LOGIN_METHOD_CHANGED,
        json!({
            "login_type": { "before": target.login_type, "after": after.login_type },
            "sessions_revoked": revoked,
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(load_detail(&state, after).await?))
}

/// Replaces a user's grants.
#[utoipa::path(
    put,
    path = "/api/users/{id}/permissions",
    tag = "users",
    params(("id" = Uuid, Path)),
    request_body = SetPermissionsRequest,
    responses(
        (status = OK, body = UserDetail),
        (status = UNPROCESSABLE_ENTITY, description = "`validation_failed`: `unknown_permission`", body = ErrorBody),
        (status = CONFLICT, description = "`cannot_change_self`", body = ErrorBody),
        (status = FORBIDDEN, description = "`forbidden`: only an admin changes an admin", body = ErrorBody),
        (status = NOT_FOUND, description = "`not_found`", body = ErrorBody),
    )
)]
pub async fn set_permissions(
    State(state): State<AppState>,
    Require(actor, _): Require<UsersManage>,
    Path(id): Path<Uuid>,
    Json(body): Json<SetPermissionsRequest>,
) -> AppResult<Json<UserDetail>> {
    let mut wanted: Vec<Permission> = Vec::new();
    let mut fields = FieldErrors::default();
    for name in &body.permissions {
        match Permission::parse(name) {
            Some(p) => wanted.push(p),
            None => fields.add("permissions", "unknown_permission"),
        }
    }
    fields.check()?;
    wanted.sort();
    wanted.dedup();

    if id == actor.user.id {
        return Err(ErrorCode::CannotChangeSelf.into());
    }
    let target = users::find(&state.db, id)
        .await?
        .ok_or(ErrorCode::NotFound)?;
    if target.role == Role::Admin && !is_admin(&actor) {
        return Err(ErrorCode::Forbidden.into());
    }

    // Both lists sorted as strings, so an unchanged set compares equal.
    let mut names: Vec<String> = wanted.iter().map(|p| p.as_str().to_string()).collect();
    names.sort();
    let mut tx = state.db.begin().await?;
    let mut before = permissions::list(&mut *tx, id).await?;
    before.sort();
    permissions::replace(&mut tx, id, &names, Some(actor.user.id)).await?;
    if before != names {
        audit::record(
            &mut *tx,
            Some(actor.user.id),
            Some(id),
            action::PERMISSIONS_CHANGED,
            json!({ "before": before, "after": names }),
        )
        .await?;
    }
    tx.commit().await?;

    Ok(Json(load_detail(&state, target).await?))
}
