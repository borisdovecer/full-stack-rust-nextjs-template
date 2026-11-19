//! Register, login, sessions, throttling, the account routes.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use sqlx::PgPool;

use app_core::config::RegistrationMode;
use common::{PASSWORD, TestApp};

use app_api::auth::repository::{Role, UserStatus};

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn register_signs_in_and_me_answers(db: PgPool) {
    let app = TestApp::new(db);
    let r = app
        .post(
            "/api/auth/register",
            json!({ "email": "  Ana@Example.COM ", "first_name": " Ana ", "last_name": " Bell ", "bio": "  ", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["user"]["email"], "ana@example.com");
    assert_eq!(r.body["user"]["first_name"], "Ana");
    assert_eq!(r.body["user"]["last_name"], "Bell");
    assert_eq!(r.body["user"]["bio"], serde_json::Value::Null);
    assert_eq!(r.body["user"]["login_type"], "password");
    assert_eq!(r.body["user"]["wallet"], serde_json::Value::Null);
    assert_eq!(r.body["user"]["role"], "user");
    assert_eq!(r.body["user"]["status"], "active");
    assert_eq!(r.body["permissions"], json!([]));
    assert_eq!(r.body["preferences"]["theme"], "system");
    assert!(r.body["user"].get("password_hash").is_none());

    let cookie = r.set_session.unwrap();
    let me = app.get("/api/me", Some(&cookie)).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["user"]["email"], "ana@example.com");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn register_reports_every_bad_field(db: PgPool) {
    let app = TestApp::new(db);
    let r = app
        .post(
            "/api/auth/register",
            json!({ "email": "no", "first_name": "", "last_name": " ", "bio": "x".repeat(1001), "password": "short" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.code(), "validation_failed");
    let fields = &r.body["error"]["details"]["fields"];
    assert_eq!(fields["email"], json!(["invalid_email"]));
    assert_eq!(fields["first_name"], json!(["required"]));
    assert_eq!(fields["last_name"], json!(["required"]));
    assert_eq!(fields["bio"], json!(["too_long"]));
    assert_eq!(fields["password"], json!(["too_short"]));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn register_missing_field_is_validation_failed(db: PgPool) {
    let app = TestApp::new(db);
    let r = app
        .post("/api/auth/register", json!({ "email": "a@b.co" }), None)
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.code(), "validation_failed");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn malformed_json_is_invalid_request(db: PgPool) {
    let app = TestApp::new(db);
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from("{not json"))
        .unwrap();
    let r = app.send(req).await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.code(), "invalid_request");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn register_taken_email_is_a_conflict(db: PgPool) {
    let app = TestApp::new(db);
    app.register("ana@example.com").await;
    let r = app
        .post(
            "/api/auth/register",
            json!({ "email": "ANA@example.com", "first_name": "Other", "last_name": "Person", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.code(), "email_taken");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn approval_mode_makes_pending_users(db: PgPool) {
    let app = TestApp::with(db, "development", |s| {
        s.registration.mode = RegistrationMode::Approval;
    });
    let cookie = app.register("pat@example.com").await;
    let me = app.get("/api/me", Some(&cookie)).await;
    assert_eq!(me.body["user"]["status"], "pending");

    let sessions = app.get("/api/me/sessions", Some(&cookie)).await;
    assert_eq!(sessions.status, StatusCode::FORBIDDEN);
    assert_eq!(sessions.code(), "account_pending");

    let theme = app
        .patch(
            "/api/me/preferences",
            json!({ "theme": "dark" }),
            Some(&cookie),
        )
        .await;
    assert_eq!(theme.status, StatusCode::OK);

    let out = app.post("/api/auth/logout", json!({}), Some(&cookie)).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn closed_mode_refuses_registration(db: PgPool) {
    let app = TestApp::with(db, "development", |s| {
        s.registration.mode = RegistrationMode::Closed;
    });
    let r = app
        .post(
            "/api/auth/register",
            json!({ "email": "a@example.com", "first_name": "A", "last_name": "B", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.code(), "registration_closed");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn wrong_password_and_unknown_email_answer_the_same(db: PgPool) {
    let app = TestApp::new(db);
    app.register("ana@example.com").await;
    let wrong = app
        .post(
            "/api/auth/login",
            json!({ "email": "ana@example.com", "password": "not-the-password" }),
            None,
        )
        .await;
    let unknown = app
        .post(
            "/api/auth/login",
            json!({ "email": "ghost@example.com", "password": "not-the-password" }),
            None,
        )
        .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.code(), "invalid_credentials");
    assert_eq!(unknown.status, wrong.status);
    assert_eq!(unknown.body["error"]["code"], wrong.body["error"]["code"]);
    assert_eq!(
        unknown.body["error"]["message"],
        wrong.body["error"]["message"]
    );
    assert!(wrong.set_session.is_none());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn login_sets_last_login_and_a_new_session(db: PgPool) {
    let app = TestApp::new(db);
    let first = app.register("ana@example.com").await;
    let second = app.login("Ana@Example.com").await;
    assert_ne!(first, second);
    let me = app.get("/api/me", Some(&second)).await;
    assert!(me.body["user"]["last_login_at"].is_string());
    let list = app.get("/api/me/sessions", Some(&second)).await;
    assert_eq!(list.body.as_array().unwrap().len(), 2);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn too_many_failures_for_an_email_are_throttled(db: PgPool) {
    let app = TestApp::new(db);
    app.register("ana@example.com").await;
    for _ in 0..5 {
        let r = app
            .post(
                "/api/auth/login",
                json!({ "email": "ana@example.com", "password": "wrong-password!!" }),
                None,
            )
            .await;
        assert_eq!(r.code(), "invalid_credentials");
    }
    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "ana@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(r.code(), "too_many_attempts");
    assert!(r.body["error"]["details"]["retry_at"].is_string());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_success_resets_the_email_count(db: PgPool) {
    let app = TestApp::new(db);
    app.register("ana@example.com").await;
    let fail = || {
        app.post(
            "/api/auth/login",
            json!({ "email": "ana@example.com", "password": "wrong-password!!" }),
            None,
        )
    };
    for _ in 0..4 {
        fail().await;
    }
    app.login("ana@example.com").await;
    for _ in 0..4 {
        assert_eq!(fail().await.code(), "invalid_credentials");
    }
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn too_many_failures_from_an_ip_are_throttled(db: PgPool) {
    let app = TestApp::with(db, "development", |s| {
        s.login_throttle.max_failures_per_ip = 3;
    });
    for i in 0..3 {
        let req = Request::post("/api/auth/login")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.7")
            .body(Body::from(
                json!({ "email": format!("u{i}@example.com"), "password": "wrong-password!!" })
                    .to_string(),
            ))
            .unwrap();
        assert_eq!(app.send(req).await.code(), "invalid_credentials");
    }
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "198.51.100.7")
        .body(Body::from(
            json!({ "email": "fresh@example.com", "password": "wrong-password!!" }).to_string(),
        ))
        .unwrap();
    assert_eq!(app.send(req).await.code(), "too_many_attempts");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_suspended_user_cannot_sign_in_and_loses_the_session(db: PgPool) {
    let app = TestApp::new(db);
    let cookie = app.register("ana@example.com").await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE email = 'ana@example.com'")
        .execute(&app.db)
        .await
        .unwrap();

    let me = app.get("/api/me", Some(&cookie)).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        me.set_session.as_deref(),
        Some(""),
        "the stale cookie is cleared"
    );

    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "ana@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.code(), "account_suspended");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_bad_or_revoked_cookie_is_unauthenticated_and_cleared(db: PgPool) {
    let app = TestApp::new(db);
    let r = app.get("/api/me", Some("garbage")).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.code(), "unauthenticated");
    assert_eq!(r.set_session.as_deref(), Some(""));

    let cookie = app.register("ana@example.com").await;
    app.post("/api/auth/logout", json!({}), Some(&cookie)).await;
    let r = app.get("/api/me", Some(&cookie)).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    let none = app.get("/api/me", None).await;
    assert_eq!(none.code(), "unauthenticated");
    assert!(none.set_session.is_none(), "no cookie, nothing to clear");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn logout_clears_the_cookie_and_logout_all_ends_every_session(db: PgPool) {
    let app = TestApp::new(db);
    let a = app.register("ana@example.com").await;
    let b = app.login("ana@example.com").await;

    let out = app.post("/api/auth/logout", json!({}), Some(&a)).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert_eq!(out.set_session.as_deref(), Some(""));
    assert_eq!(app.get("/api/me", Some(&b)).await.status, StatusCode::OK);

    let c = app.login("ana@example.com").await;
    let all = app.post("/api/auth/logout-all", json!({}), Some(&c)).await;
    assert_eq!(all.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/me", Some(&b)).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/api/me", Some(&c)).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn change_password_keeps_this_session_and_ends_the_others(db: PgPool) {
    let app = TestApp::new(db);
    let a = app.register("ana@example.com").await;
    let b = app.login("ana@example.com").await;

    let wrong = app
        .post(
            "/api/me/password",
            json!({ "current_password": "not-the-password", "new_password": "brand-new-password" }),
            Some(&a),
        )
        .await;
    assert_eq!(wrong.code(), "wrong_password");

    let weak = app
        .post(
            "/api/me/password",
            json!({ "current_password": PASSWORD, "new_password": "short" }),
            Some(&a),
        )
        .await;
    assert_eq!(
        weak.body["error"]["details"]["fields"]["new_password"],
        json!(["too_short"])
    );

    let ok = app
        .post(
            "/api/me/password",
            json!({ "current_password": PASSWORD, "new_password": "brand-new-password" }),
            Some(&a),
        )
        .await;
    assert_eq!(ok.status, StatusCode::NO_CONTENT);
    assert_eq!(app.get("/api/me", Some(&a)).await.status, StatusCode::OK);
    assert_eq!(
        app.get("/api/me", Some(&b)).await.status,
        StatusCode::UNAUTHORIZED
    );

    let new_login = app
        .post(
            "/api/auth/login",
            json!({ "email": "ana@example.com", "password": "brand-new-password" }),
            None,
        )
        .await;
    assert_eq!(new_login.status, StatusCode::OK);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn wrong_current_passwords_are_throttled_with_logins(db: PgPool) {
    let app = TestApp::new(db);
    let a = app.register("ana@example.com").await;
    let change = |current: &'static str| {
        app.post(
            "/api/me/password",
            json!({ "current_password": current, "new_password": "brand-new-password" }),
            Some(&a),
        )
    };
    for _ in 0..5 {
        assert_eq!(change("wrong-password!!").await.code(), "wrong_password");
    }
    let r = change(PASSWORD).await;
    assert_eq!(r.code(), "too_many_attempts");
    assert!(r.body["error"]["details"]["retry_at"].is_string());

    // The same count guards the login form.
    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "ana@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.code(), "too_many_attempts");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn sessions_list_and_revoke(db: PgPool) {
    let app = TestApp::new(db);
    let a = app.register("ana@example.com").await;
    let b = app.login("ana@example.com").await;
    let other = app.register("bo@example.com").await;

    let list = app.get("/api/me/sessions", Some(&a)).await.body;
    let list = list.as_array().unwrap();
    assert_eq!(list.len(), 2);
    let current = list.iter().find(|s| s["current"] == true).unwrap();
    let not_current = list.iter().find(|s| s["current"] == false).unwrap();
    let not_current_id = not_current["id"].as_str().unwrap();

    let foreign = app
        .delete(&format!("/api/me/sessions/{not_current_id}"), Some(&other))
        .await;
    assert_eq!(foreign.code(), "not_found");

    let r = app
        .delete(&format!("/api/me/sessions/{not_current_id}"), Some(&a))
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/me", Some(&b)).await.status,
        StatusCode::UNAUTHORIZED
    );

    let current_id = current["id"].as_str().unwrap();
    let r = app
        .delete(&format!("/api/me/sessions/{current_id}"), Some(&a))
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(r.set_session.as_deref(), Some(""));

    let bad = app
        .delete("/api/me/sessions/not-a-uuid", Some(&other))
        .await;
    assert_eq!(bad.status, StatusCode::NOT_FOUND);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn profile_and_preferences(db: PgPool) {
    let app = TestApp::new(db);
    let c = app.register("ana@example.com").await;
    let r = app
        .patch(
            "/api/me",
            json!({ "first_name": "  Ana ", "last_name": " B ", "bio": " Hi " }),
            Some(&c),
        )
        .await;
    assert_eq!(r.body["user"]["first_name"], "Ana");
    assert_eq!(r.body["user"]["last_name"], "B");
    assert_eq!(r.body["user"]["bio"], "Hi");
    // A field left out stays; an empty bio clears it.
    let r = app.patch("/api/me", json!({ "bio": "" }), Some(&c)).await;
    assert_eq!(r.body["user"]["first_name"], "Ana");
    assert_eq!(r.body["user"]["bio"], serde_json::Value::Null);
    let r = app
        .patch("/api/me", json!({ "last_name": "" }), Some(&c))
        .await;
    assert_eq!(r.code(), "validation_failed");
    let r = app
        .patch("/api/me", json!({ "bio": "x".repeat(1001) }), Some(&c))
        .await;
    assert_eq!(r.code(), "validation_failed");

    let r = app
        .patch("/api/me/preferences", json!({ "theme": "dark" }), Some(&c))
        .await;
    assert_eq!(r.body["preferences"]["theme"], "dark");
    let r = app
        .patch("/api/me/preferences", json!({ "theme": "neon" }), Some(&c))
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_used_session_is_extended_and_its_cookie_refreshed(db: PgPool) {
    let app = TestApp::with(db, "development", |s| {
        s.session.extend_after_hours = 0;
    });
    let c = app.register("ana@example.com").await;
    let r = app.get("/api/me", Some(&c)).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.set_session.as_deref(), Some(c.as_str()));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn admins_get_every_permission(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let me = app.get("/api/me", Some(&admin)).await;
    assert_eq!(
        me.body["permissions"],
        json!(["users.read", "users.manage"])
    );
    let (_, user) = app
        .user_with("u@example.com", Role::User, UserStatus::Active)
        .await;
    assert_eq!(
        app.get("/api/me", Some(&user)).await.body["permissions"],
        json!([])
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn housekeeping_removes_ended_sessions(db: PgPool) {
    let app = TestApp::new(db);
    let c = app.register("ana@example.com").await;
    app.post("/api/auth/logout", json!({}), Some(&c)).await;
    sqlx::query("UPDATE user_sessions SET revoked_at = now() - interval '2 days'")
        .execute(&app.db)
        .await
        .unwrap();
    app_api::jobs::housekeeping::run_once(&app.db).await;
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM user_sessions")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn parallel_wrong_passwords_stay_within_the_limit(db: PgPool) {
    let app = TestApp::new(db);
    app.register("par@example.com").await;
    let tries = (0..20)
        .map(|i| {
            app.post(
                "/api/auth/login",
                json!({ "email": "par@example.com", "password": format!("wrong-password-{i}") }),
                None,
            )
        })
        .collect();
    let answers = common::join_all(tries).await;
    let wrong = answers
        .iter()
        .filter(|r| r.code() == "invalid_credentials")
        .count();
    let paused = answers
        .iter()
        .filter(|r| r.code() == "too_many_attempts")
        .count();
    assert_eq!(wrong + paused, 20);
    assert!(wrong <= 5, "{wrong} passwords were checked, the limit is 5");

    let failures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_login_attempts WHERE email = $1 AND NOT success",
    )
    .bind("par@example.com")
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert_eq!(failures, i64::try_from(wrong).unwrap());
}
