//! Sessions on requests: the cookie, the lookup, and the extractors handlers use.
//!
//! The `session` middleware runs on every request. It finds the session for the cookie,
//! extends it when it is due, clears a cookie that no longer matches a session, and leaves an
//! [`Authenticated`] in the request for the extractors. Handlers never read the cookie.
//! When the lookup itself fails (database down or busy), the request goes on without a user,
//! marked [`SessionUnavailable`]: public routes still answer, and routes that need a session
//! answer `503 unavailable` rather than `401`, so nobody is signed out by an outage.

use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

pub use app_auth::permission::marker::*;
use app_auth::{Permission, RequiredPermission, permission, token};
use app_core::{AppError, AppResult, ErrorCode};

use super::dto::Me;
use super::repository::{Role, User, UserStatus, permissions, sessions};
use crate::state::AppState;

/// How often a session's `last_seen_at` is written when nothing else changes.
const TOUCH_EVERY: Duration = Duration::minutes(5);
const USER_AGENT_MAX: usize = 255;

/// Put in the request when the session could not be looked up.
#[derive(Debug, Clone, Copy)]
pub struct SessionUnavailable;

/// The signed-in user and their session, put in the request by [`session`].
#[derive(Debug, Clone)]
pub struct Authenticated {
    pub session_id: Uuid,
    /// When this session signed in; a sensitive change asks for a recent one.
    pub session_created_at: DateTime<Utc>,
    pub user: User,
}

pub async fn session(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let settings = &state.config.settings.session;
    let token = read_cookie(req.headers(), &settings.cookie_name);
    let mut refresh: Option<String> = None;
    let mut clear = false;

    if let Some(token) = token {
        let found = if token::looks_valid(&token) {
            match sessions::find_valid(&state.db, &token::hash(&token)).await {
                Ok(found) => Ok(found),
                Err(e) => {
                    tracing::warn!(error = %e, "cannot look up the session");
                    Err(())
                }
            }
        } else {
            Ok(None)
        };

        match found {
            Err(()) => {
                req.extensions_mut().insert(SessionUnavailable);
            }
            Ok(Some(s)) if s.user.status != UserStatus::Suspended => {
                let now = Utc::now();
                let lifetime = Duration::hours(i64::from(settings.lifetime_hours));
                let extend_after = Duration::hours(i64::from(settings.extend_after_hours));
                let issued = s.session_expires_at - lifetime;
                let new_expiry = (now - issued >= extend_after).then(|| now + lifetime);
                if new_expiry.is_some() || now - s.session_last_seen_at >= TOUCH_EVERY {
                    if let Err(e) = sessions::touch(&state.db, s.session_id, new_expiry).await {
                        tracing::warn!(error = %e, "cannot touch session");
                    } else if new_expiry.is_some() {
                        refresh = Some(token);
                    }
                }
                req.extensions_mut().insert(Authenticated {
                    session_id: s.session_id,
                    session_created_at: s.session_created_at,
                    user: s.user,
                });
            }
            Ok(_) => clear = true,
        }
    }

    let mut res = next.run(req).await;
    if handler_set_cookie(res.headers(), &settings.cookie_name) {
        return res;
    }
    if let Some(token) = refresh {
        append_cookie(&mut res, session_cookie(&state, token));
    } else if clear {
        append_cookie(&mut res, removal_cookie(&state));
    }
    res
}

pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| Cookie::parse(pair.trim().to_string()).ok())
        .find(|c| c.name() == name)
        .map(|c| c.value().to_string())
}

fn handler_set_cookie(headers: &HeaderMap, name: &str) -> bool {
    let prefix = format!("{name}=");
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| v.starts_with(&prefix))
}

pub fn append_cookie(res: &mut Response, cookie: Cookie<'static>) {
    if let Ok(v) = HeaderValue::from_str(&cookie.to_string()) {
        res.headers_mut().append(header::SET_COOKIE, v);
    }
}

/// The session cookie for a token, valid for the session's lifetime.
pub fn session_cookie(state: &AppState, token: String) -> Cookie<'static> {
    let settings = &state.config.settings.session;
    Cookie::build((settings.cookie_name.clone(), token))
        .http_only(true)
        .secure(state.config.env.is_production())
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time_hours(settings.lifetime_hours))
        .build()
}

/// A cookie that deletes the session cookie in the browser.
pub fn removal_cookie(state: &AppState) -> Cookie<'static> {
    let mut c = session_cookie(state, String::new());
    c.make_removal();
    c
}

fn time_hours(hours: u32) -> cookie::time::Duration {
    cookie::time::Duration::hours(i64::from(hours))
}

/// A new session for a user; returns the cookie to set.
pub async fn start_session(
    state: &AppState,
    user_id: Uuid,
    client: &Client,
) -> Result<Cookie<'static>, AppError> {
    let (token, hash) = token::generate();
    let expires_at =
        Utc::now() + Duration::hours(i64::from(state.config.settings.session.lifetime_hours));
    sessions::create(
        &state.db,
        sessions::NewSession {
            user_id,
            token_hash: &hash,
            expires_at,
            ip: client.ip.as_deref(),
            user_agent: client.user_agent.as_deref(),
        },
    )
    .await?;
    Ok(session_cookie(state, token))
}

/// Who is calling: the IP and the user agent, for sessions and throttling.
#[derive(Debug, Clone, Default)]
pub struct Client {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

impl<S: Send + Sync> FromRequestParts<S> for Client {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        let user_agent = parts
            .headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.chars().take(USER_AGENT_MAX).collect());
        Ok(Client {
            ip: client_ip(peer, &parts.headers).map(|ip| ip.to_string()),
            user_agent,
        })
    }
}

/// The caller's IP. A loopback peer is a proxy on this machine (Caddy, or Next in development),
/// so the address it saw is the last entry of `X-Forwarded-For`. Anyone else is the client.
fn client_ip(peer: Option<IpAddr>, headers: &HeaderMap) -> Option<IpAddr> {
    let forwarded = || {
        headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .and_then(|v| v.trim().parse::<IpAddr>().ok())
    };
    match peer {
        Some(ip) if ip.is_loopback() => forwarded().or(Some(ip)),
        Some(ip) => Some(ip),
        None => forwarded(),
    }
}

/// Any signed-in user, `pending` included. For the few routes a pending user may use.
#[derive(Debug, Clone)]
pub struct SessionUser(pub Authenticated);

impl<S: Send + Sync> FromRequestParts<S> for SessionUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Authenticated>()
            .cloned()
            .map(SessionUser)
            .ok_or_else(|| {
                if parts.extensions.get::<SessionUnavailable>().is_some() {
                    AppError::new(ErrorCode::Unavailable)
                } else {
                    AppError::new(ErrorCode::Unauthenticated)
                }
            })
    }
}

/// A signed-in, active user. Use this unless a route is meant for pending users too.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub Authenticated);

impl<S: Send + Sync> FromRequestParts<S> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let SessionUser(auth) = SessionUser::from_request_parts(parts, state).await?;
        match auth.user.status {
            UserStatus::Active => Ok(CurrentUser(auth)),
            UserStatus::Pending => Err(AppError::new(ErrorCode::AccountPending)),
            UserStatus::Suspended => Err(AppError::new(ErrorCode::AccountSuspended)),
        }
    }
}

/// An active user who has permission `P`, a marker from `app_auth::permission::marker`
/// (re-exported here): `Require<UsersRead>`.
pub struct Require<P: RequiredPermission>(pub Authenticated, pub PhantomData<P>);

impl<P: RequiredPermission> FromRequestParts<AppState> for Require<P> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let CurrentUser(auth) = CurrentUser::from_request_parts(parts, state).await?;
        let granted = effective_permissions(state, &auth.user).await?;
        if granted.contains(&P::PERMISSION) {
            Ok(Require(auth, PhantomData))
        } else {
            Err(AppError::new(ErrorCode::Forbidden))
        }
    }
}

/// Role defaults plus grants. Names the code no longer knows are skipped.
pub async fn effective_permissions(
    state: &AppState,
    user: &User,
) -> Result<Vec<Permission>, AppError> {
    let grants = permissions::list(&state.db, user.id).await?;
    Ok(combine_permissions(user, &grants))
}

/// Role defaults plus the named grants, sorted, each once.
pub fn combine_permissions<S: AsRef<str>>(user: &User, grants: &[S]) -> Vec<Permission> {
    let mut all: Vec<Permission> = permission::role_defaults(user.role == Role::Admin).to_vec();
    for name in grants {
        let name = name.as_ref();
        match Permission::parse(name) {
            Some(p) => all.push(p),
            None => tracing::warn!(permission = %name, user = %user.id, "unknown permission"),
        }
    }
    all.sort();
    all.dedup();
    all
}

/// Whoever holds a signed-in browser must not be able to take the account over, so changing
/// how it signs in needs a session that signed in within `session.reauth_minutes`.
pub fn require_recent_sign_in(state: &AppState, auth: &Authenticated) -> Result<(), ErrorCode> {
    let minutes = i64::from(state.config.settings.session.reauth_minutes);
    if auth.session_created_at < Utc::now() - Duration::minutes(minutes) {
        return Err(ErrorCode::ReauthRequired);
    }
    Ok(())
}

/// `204` that clears the session cookie.
pub fn signed_out(state: &AppState) -> Response {
    let mut res = axum::http::StatusCode::NO_CONTENT.into_response();
    append_cookie(&mut res, removal_cookie(state));
    res
}

/// The signed-in user, what they may do, and their preferences.
pub async fn load_me(state: &AppState, user: User) -> AppResult<Me> {
    let permissions = effective_permissions(state, &user).await?;
    let prefs = super::repository::preferences::get(&state.db, user.id).await?;
    Ok(Me {
        user: user.into(),
        permissions,
        preferences: prefs.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(xff: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(v) = xff {
            h.insert("x-forwarded-for", HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn forwarded_for_is_trusted_only_from_loopback() {
        let local: IpAddr = "127.0.0.1".parse().unwrap();
        let remote: IpAddr = "203.0.113.9".parse().unwrap();
        let h = headers(Some("1.1.1.1, 198.51.100.7"));
        assert_eq!(client_ip(Some(local), &h), "198.51.100.7".parse().ok());
        assert_eq!(client_ip(Some(remote), &h), Some(remote));
        assert_eq!(client_ip(Some(local), &headers(None)), Some(local));
        assert_eq!(client_ip(Some(local), &headers(Some("junk"))), Some(local));
    }

    #[test]
    fn cookie_is_read_among_others() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; app_session=tok; b=2"),
        );
        assert_eq!(read_cookie(&h, "app_session").as_deref(), Some("tok"));
        assert_eq!(read_cookie(&h, "missing"), None);
    }
}
