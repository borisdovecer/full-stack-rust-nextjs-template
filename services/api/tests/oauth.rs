//! The `external` sign-in method against a fake Google and GitHub: registering, signing in,
//! switching from Settings, and every way the round trip is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use sqlx::PgPool;

use app_core::config::{OauthEndpoints, Secret};
use common::{PASSWORD, TestApp};

use app_api::auth::repository::users::LoginMethod;
use app_api::auth::repository::{Role, UserStatus};

/// A provider account the fake hands out for a code: the PKCE challenge it expects, and what
/// its user endpoints answer.
#[derive(Clone)]
struct Grant {
    challenge: String,
    user: Value,
    emails: Value,
}

type Grants = Arc<Mutex<HashMap<String, Grant>>>;

struct Fake {
    url: String,
    grants: Grants,
}

async fn fake_provider() -> Fake {
    let grants = Grants::default();
    let app = Router::new()
        .route("/token", post(token))
        .route("/user", get(user))
        .route("/emails", get(emails))
        .with_state(grants.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await });
    Fake {
        url: format!("http://{addr}"),
        grants,
    }
}

async fn token(
    State(grants): State<Grants>,
    Form(form): Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let code = form.get("code").cloned().unwrap_or_default();
    let grant = grants.lock().unwrap().get(&code).cloned();
    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
    match grant {
        Some(g)
            if form.get("client_secret").map(String::as_str) == Some("secret")
                && app_auth::pkce::challenge(&verifier) == g.challenge =>
        {
            (StatusCode::OK, Json(json!({ "access_token": code })))
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid_grant" })),
        ),
    }
}

fn bearer(headers: &HeaderMap) -> String {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string()
}

async fn user(State(grants): State<Grants>, headers: HeaderMap) -> Json<Value> {
    let g = grants
        .lock()
        .unwrap()
        .get(&bearer(&headers))
        .cloned()
        .unwrap();
    Json(g.user)
}

async fn emails(State(grants): State<Grants>, headers: HeaderMap) -> Json<Value> {
    let g = grants
        .lock()
        .unwrap()
        .get(&bearer(&headers))
        .cloned()
        .unwrap();
    Json(g.emails)
}

fn app(db: PgPool, fake: &Fake, tweak: impl FnOnce(&mut app_core::Config)) -> TestApp {
    let endpoints = OauthEndpoints {
        authorize_url: format!("{}/authorize", fake.url),
        token_url: format!("{}/token", fake.url),
        user_url: format!("{}/user", fake.url),
        emails_url: Some(format!("{}/emails", fake.url)),
    };
    TestApp::with_config(db, "development", |c| {
        c.google_client_secret = Some(Secret::new("secret"));
        c.github_client_secret = Some(Secret::new("secret"));
        c.settings.oauth.google.client_id = "google-id".into();
        c.settings.oauth.google.endpoints = Some(endpoints.clone());
        c.settings.oauth.github.client_id = "github-id".into();
        c.settings.oauth.github.endpoints = Some(endpoints);
        tweak(c);
    })
}

fn google(sub: &str, email: &str, verified: bool) -> (Value, Value) {
    let user = json!({
        "sub": sub, "email": email, "email_verified": verified,
        "given_name": "Ana", "family_name": "Lee",
    });
    (user, Value::Null)
}

fn github(id: u64, email: &str, verified: bool) -> (Value, Value) {
    (
        json!({ "id": id, "login": "ana", "name": "Ana Maria Lee" }),
        json!([
            { "email": "other@example.com", "primary": false, "verified": true },
            { "email": email, "primary": true, "verified": verified },
        ]),
    )
}

/// One round trip: start, the provider "approves" with `account`, the callback.
struct Trip {
    flow: String,
    state: String,
}

async fn start(app: &TestApp, provider: &str, intent: &str, session: Option<&str>) -> common::Res {
    let mut req = Request::get(format!("/api/auth/oauth/{provider}/start?intent={intent}"));
    if let Some(s) = session {
        req = req.header(header::COOKIE, format!("app_session={s}"));
    }
    app.send(req.body(Body::empty()).unwrap()).await
}

async fn begin(
    app: &TestApp,
    fake: &Fake,
    provider: &str,
    intent: &str,
    session: Option<&str>,
    account: (Value, Value),
    code: &str,
) -> Trip {
    let r = start(app, provider, intent, session).await;
    assert_eq!(r.status, StatusCode::SEE_OTHER, "{}", r.body);
    let url = reqwest::Url::parse(r.location()).unwrap();
    let q: HashMap<_, _> = url.query_pairs().into_owned().collect();
    fake.grants.lock().unwrap().insert(
        code.to_string(),
        Grant {
            challenge: q["code_challenge"].clone(),
            user: account.0,
            emails: account.1,
        },
    );
    Trip {
        flow: r.cookie("app_session_oauth").unwrap(),
        state: q["state"].clone(),
    }
}

async fn finish(
    app: &TestApp,
    provider: &str,
    trip: &Trip,
    code: &str,
    session: Option<&str>,
) -> common::Res {
    let mut cookies = vec![format!("app_session_oauth={}", trip.flow)];
    if let Some(s) = session {
        cookies.push(format!("app_session={s}"));
    }
    let req = Request::get(format!(
        "/api/auth/oauth/{provider}/callback?code={code}&state={}",
        trip.state
    ))
    .header(header::COOKIE, cookies.join("; "))
    .body(Body::empty())
    .unwrap();
    app.send(req).await
}

/// Registers through the provider and the form; returns the session cookie.
async fn register_via(
    app: &TestApp,
    fake: &Fake,
    provider: &str,
    account: (Value, Value),
    code: &str,
) -> String {
    let trip = begin(app, fake, provider, "register", None, account, code).await;
    let r = finish(app, provider, &trip, code, None).await;
    assert_eq!(r.location(), "/register?via=external");
    let signup = r.cookie("app_session_signup").unwrap();
    let req = Request::post("/api/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("app_session_signup={signup}"))
        .body(Body::from(
            json!({ "method": "external", "email": "ignored@example.com", "first_name": "Ana", "last_name": "Lee" })
                .to_string(),
        ))
        .unwrap();
    let r = app.send(req).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    r.set_session.unwrap()
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn providers_are_listed_only_when_configured(db: PgPool) {
    let off = TestApp::new(db.clone());
    let r = off.get("/api/config", None).await;
    assert_eq!(r.body["oauth_providers"], json!([]));
    assert!(
        !r.body["login_methods"]
            .as_array()
            .unwrap()
            .contains(&json!("external"))
    );
    assert_eq!(
        start(&off, "google", "login", None).await.status,
        StatusCode::NOT_FOUND
    );

    let fake = fake_provider().await;
    let on = app(db, &fake, |c| c.github_client_secret = None);
    let r = on.get("/api/config", None).await;
    assert_eq!(r.body["oauth_providers"], json!(["google"]));
    assert!(
        r.body["login_methods"]
            .as_array()
            .unwrap()
            .contains(&json!("external"))
    );
    assert!(
        !r.body["admin_login_methods"]
            .as_array()
            .unwrap()
            .contains(&json!("external"))
    );
    assert_eq!(
        start(&on, "github", "login", None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        start(&on, "myspace", "login", None).await.status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn start_sends_the_browser_with_state_and_pkce(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});
    let r = start(&app, "google", "login", None).await;
    let url = reqwest::Url::parse(r.location()).unwrap();
    let q: HashMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(q["client_id"], "google-id");
    assert_eq!(
        q["redirect_uri"],
        "http://127.0.0.1:3000/api/auth/oauth/google/callback"
    );
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["code_challenge_method"], "S256");
    assert!(q["scope"].contains("email"));
    let flow = r.cookie("app_session_oauth").unwrap();
    assert!(flow.starts_with("google.login."));
    assert!(flow.contains(&q["state"]));
    let set = r.headers.get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(set.contains("HttpOnly") && set.contains("Path=/api/auth/oauth"));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_google_account_registers_then_signs_in(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});

    // The form reads what the provider vouched for.
    let trip = begin(
        &app,
        &fake,
        "google",
        "register",
        None,
        google("g-1", "Ana@Example.com", true),
        "c1",
    )
    .await;
    let r = finish(&app, "google", &trip, "c1", None).await;
    let signup = r.cookie("app_session_signup").unwrap();
    assert_eq!(r.cookie("app_session_oauth").as_deref(), Some(""));
    let req = Request::get("/api/auth/oauth/signup")
        .header(header::COOKIE, format!("app_session_signup={signup}"))
        .body(Body::empty())
        .unwrap();
    let pending = app.send(req).await;
    assert_eq!(
        pending.body,
        json!({ "provider": "google", "email": "ana@example.com", "first_name": "Ana", "last_name": "Lee" })
    );

    let body = json!({ "method": "external", "email": "x@example.com", "first_name": "Ana", "last_name": "Lee" });
    let post = |cookie: Option<&str>| {
        let mut req =
            Request::post("/api/auth/register").header(header::CONTENT_TYPE, "application/json");
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, format!("app_session_signup={c}"));
        }
        req.body(Body::from(body.to_string())).unwrap()
    };
    let r = app.send(post(None)).await;
    assert_eq!(r.code(), "provider_failed");
    let r = app.send(post(Some(&signup))).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["user"]["email"], "ana@example.com");
    assert_eq!(r.body["user"]["login_type"], "external");
    assert_eq!(r.cookie("app_session_signup").as_deref(), Some(""));
    // The waiting answer is used once.
    assert_eq!(
        app.send(post(Some(&signup))).await.code(),
        "provider_failed"
    );

    let trip = begin(
        &app,
        &fake,
        "google",
        "login",
        None,
        google("g-1", "ana@example.com", true),
        "c2",
    )
    .await;
    let r = finish(&app, "google", &trip, "c2", None).await;
    assert_eq!(r.location(), "/");
    let session = r.set_session.unwrap();
    let me = app.get("/api/me", Some(&session)).await;
    assert_eq!(me.body["user"]["email"], "ana@example.com");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn the_account_follows_the_provider_id_not_the_email(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});
    register_via(
        &app,
        &fake,
        "github",
        github(42, "ana@example.com", true),
        "c1",
    )
    .await;

    // Same GitHub account, new email: still the same account.
    let trip = begin(
        &app,
        &fake,
        "github",
        "login",
        None,
        github(42, "ana@new.example", true),
        "c2",
    )
    .await;
    let r = finish(&app, "github", &trip, "c2", None).await;
    assert_eq!(r.location(), "/");

    // Another GitHub account with the old email: not this account.
    let trip = begin(
        &app,
        &fake,
        "github",
        "login",
        None,
        github(7, "ana@example.com", true),
        "c3",
    )
    .await;
    let r = finish(&app, "github", &trip, "c3", None).await;
    assert_eq!(r.location(), "/login?error=invalid_credentials");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn unknown_accounts_and_other_methods_cannot_sign_in(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});
    app.register("pw@example.com").await;
    for (email, code) in [("nobody@example.com", "c1"), ("pw@example.com", "c2")] {
        let trip = begin(
            &app,
            &fake,
            "google",
            "login",
            None,
            google(code, email, true),
            code,
        )
        .await;
        let r = finish(&app, "google", &trip, code, None).await;
        assert_eq!(r.location(), "/login?error=invalid_credentials", "{email}");
        assert!(r.set_session.is_none());
    }
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_external_account_without_an_identity_is_tied_by_its_verified_email(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});
    let (id, _) = app
        .user_with("tie@example.com", Role::User, UserStatus::Active)
        .await;
    app.set_login_type(id, LoginMethod::External).await;

    let trip = begin(
        &app,
        &fake,
        "google",
        "login",
        None,
        google("g-9", "tie@example.com", true),
        "c1",
    )
    .await;
    assert_eq!(
        finish(&app, "google", &trip, "c1", None).await.location(),
        "/"
    );
    let tied: String = sqlx::query_scalar("SELECT subject FROM user_identities WHERE user_id = $1")
        .bind(id)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(tied, "g-9");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn the_round_trip_is_refused_when_anything_is_off(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});
    register_via(
        &app,
        &fake,
        "google",
        google("g-1", "ana@example.com", true),
        "c0",
    )
    .await;

    // A state that does not match the cookie.
    let mut trip = begin(
        &app,
        &fake,
        "google",
        "login",
        None,
        google("g-1", "ana@example.com", true),
        "c1",
    )
    .await;
    trip.state = "forged".into();
    let r = finish(&app, "google", &trip, "c1", None).await;
    assert_eq!(r.location(), "/login?error=provider_failed");
    assert!(r.set_session.is_none());

    // No flow cookie at all (another browser).
    let req = Request::get("/api/auth/oauth/google/callback?code=c1&state=x")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.send(req).await.location(),
        "/login?error=provider_failed"
    );

    // A flow cookie for another provider.
    let trip = begin(
        &app,
        &fake,
        "github",
        "login",
        None,
        github(1, "ana@example.com", true),
        "c2",
    )
    .await;
    assert_eq!(
        finish(&app, "google", &trip, "c2", None).await.location(),
        "/login?error=provider_failed"
    );

    // A verifier that does not match the challenge: the provider refuses the code.
    let mut trip = begin(
        &app,
        &fake,
        "google",
        "login",
        None,
        google("g-1", "ana@example.com", true),
        "c3",
    )
    .await;
    let mut parts: Vec<&str> = trip.flow.split('.').collect();
    parts[3] = "not-the-verifier";
    trip.flow = parts.join(".");
    assert_eq!(
        finish(&app, "google", &trip, "c3", None).await.location(),
        "/login?error=provider_failed"
    );

    // The person cancelled at the provider.
    let trip = begin(
        &app,
        &fake,
        "google",
        "register",
        None,
        google("g-1", "ana@example.com", true),
        "c4",
    )
    .await;
    let req = Request::get(format!(
        "/api/auth/oauth/google/callback?error=access_denied&state={}",
        trip.state
    ))
    .header(header::COOKIE, format!("app_session_oauth={}", trip.flow))
    .body(Body::empty())
    .unwrap();
    assert_eq!(app.send(req).await.location(), "/register");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_email_the_provider_has_not_verified_is_refused(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db, &fake, |_| {});
    let trip = begin(
        &app,
        &fake,
        "github",
        "register",
        None,
        github(5, "new@example.com", false),
        "c1",
    )
    .await;
    let r = finish(&app, "github", &trip, "c1", None).await;
    assert_eq!(r.location(), "/register?error=email_not_verified");
    assert!(r.cookie("app_session_signup").is_none());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn registering_refuses_taken_emails_and_closed_registration(db: PgPool) {
    let fake = fake_provider().await;
    let app1 = app(db.clone(), &fake, |_| {});
    app1.register("taken@example.com").await;
    let trip = begin(
        &app1,
        &fake,
        "google",
        "register",
        None,
        google("g-2", "taken@example.com", true),
        "c1",
    )
    .await;
    assert_eq!(
        finish(&app1, "google", &trip, "c1", None).await.location(),
        "/register?error=email_taken"
    );

    let closed = app(db, &fake, |c| {
        c.settings.registration.mode = app_core::config::RegistrationMode::Closed;
    });
    let trip = begin(
        &closed,
        &fake,
        "google",
        "register",
        None,
        google("g-3", "x@example.com", true),
        "c2",
    )
    .await;
    assert_eq!(
        finish(&closed, "google", &trip, "c2", None)
            .await
            .location(),
        "/register?error=registration_closed"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_suspended_account_is_refused(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db.clone(), &fake, |_| {});
    register_via(
        &app,
        &fake,
        "google",
        google("g-1", "s@example.com", true),
        "c1",
    )
    .await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE email = 's@example.com'")
        .execute(&db)
        .await
        .unwrap();
    let trip = begin(
        &app,
        &fake,
        "google",
        "login",
        None,
        google("g-1", "s@example.com", true),
        "c2",
    )
    .await;
    assert_eq!(
        finish(&app, "google", &trip, "c2", None).await.location(),
        "/login?error=account_suspended"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_password_account_switches_to_google_from_settings(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db.clone(), &fake, |_| {});
    let (id, session) = app
        .user_with("me@example.com", Role::User, UserStatus::Active)
        .await;
    let other = app.login("me@example.com").await;

    // Another email at the provider is refused.
    let trip = begin(
        &app,
        &fake,
        "google",
        "link",
        Some(&session),
        google("g-1", "else@example.com", true),
        "c1",
    )
    .await;
    assert_eq!(
        finish(&app, "google", &trip, "c1", Some(&session))
            .await
            .location(),
        "/settings?error=provider_email_mismatch"
    );

    let trip = begin(
        &app,
        &fake,
        "google",
        "link",
        Some(&session),
        google("g-1", "me@example.com", true),
        "c2",
    )
    .await;
    let r = finish(&app, "google", &trip, "c2", Some(&session)).await;
    assert_eq!(r.location(), "/settings?changed=external");
    let me = app.get("/api/me", Some(&session)).await;
    assert_eq!(me.body["user"]["login_type"], "external");
    assert_eq!(
        app.get("/api/me", Some(&other)).await.status,
        StatusCode::UNAUTHORIZED
    );
    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "me@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.code(), "invalid_credentials");

    // Already external: nothing to switch.
    let r = start(&app, "github", "link", Some(&session)).await;
    assert_eq!(r.location(), "/settings?error=login_method_unchanged");

    // An admin moving them back to a password unties the provider account.
    let (_, admin) = app.admin().await;
    let r = app
        .put(
            &format!("/api/users/{id}/login-method"),
            json!({ "method": "password", "password": "recovered-password" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM user_identities")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn switching_needs_a_recent_signed_in_session(db: PgPool) {
    let fake = fake_provider().await;
    let app = app(db.clone(), &fake, |_| {});
    assert_eq!(
        start(&app, "google", "link", None).await.location(),
        "/settings?error=unauthenticated"
    );
    let (_, session) = app
        .user_with("me@example.com", Role::User, UserStatus::Active)
        .await;
    sqlx::query("UPDATE user_sessions SET created_at = now() - interval '11 minutes'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        start(&app, "google", "link", Some(&session))
            .await
            .location(),
        "/settings?error=reauth_required"
    );
}
