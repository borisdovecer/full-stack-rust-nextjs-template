//! The `code` sign-in method: asking for a code, signing in, registering and switching with
//! one, and the limits that keep codes from being guessed or used to flood an inbox.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use sqlx::PgPool;

use app_api::auth::repository::users::LoginMethod;
use app_api::auth::repository::{Role, UserStatus};
use common::{PASSWORD, TestApp};

/// A `code` account, created directly in the database.
async fn code_user(app: &TestApp, email: &str) -> uuid::Uuid {
    let (id, _) = app.user_with(email, Role::User, UserStatus::Active).await;
    app.set_login_type(id, LoginMethod::Code).await;
    id
}

async fn ask(app: &TestApp, email: &str) -> StatusCode {
    app.post("/api/auth/code", json!({ "email": email }), None)
        .await
        .status
}

async fn login(app: &TestApp, email: &str, code: &str) -> common::Res {
    app.post(
        "/api/auth/login",
        json!({ "method": "code", "email": email, "login_code": code }),
        None,
    )
    .await
}

/// A six-digit code that is not `code`.
fn other_than(code: &str) -> String {
    let n: u32 = code.parse().unwrap();
    format!("{:06}", (n + 1) % 1_000_000)
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_code_signs_in_once(db: PgPool) {
    let app = TestApp::new(db);
    code_user(&app, "c@example.com").await;

    assert_eq!(ask(&app, "C@Example.com ").await, StatusCode::NO_CONTENT);
    let code = app.code_for("c@example.com", 1).await;
    let mail = app.mail_to("c@example.com", 1).await;
    assert!(mail[0].subject.contains("sign-in code"));

    let r = login(&app, "c@example.com", &code).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "code");
    assert!(r.set_session.is_some());

    let again = login(&app, "c@example.com", &code).await;
    assert_eq!(again.status, StatusCode::UNAUTHORIZED);
    assert_eq!(again.code(), "invalid_code");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn asking_answers_the_same_for_everyone_and_mails_only_code_accounts(db: PgPool) {
    let app = TestApp::new(db);
    app.register("pw@example.com").await;
    let suspended = code_user(&app, "gone@example.com").await;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(suspended)
        .execute(&app.db)
        .await
        .unwrap();

    for email in ["nobody@example.com", "pw@example.com", "gone@example.com"] {
        let r = app
            .post("/api/auth/code", json!({ "email": email }), None)
            .await;
        assert_eq!(r.status, StatusCode::NO_CONTENT, "{email}");
        assert_eq!(r.body, serde_json::Value::Null);
    }
    app.no_mail_to("nobody@example.com").await;
    app.no_mail_to("pw@example.com").await;
    app.no_mail_to("gone@example.com").await;

    // A password account cannot sign in with a code, and says so like any wrong code.
    let r = login(&app, "pw@example.com", "123456").await;
    assert_eq!(r.code(), "invalid_code");
    let r = login(&app, "nobody@example.com", "123456").await;
    assert_eq!(r.code(), "invalid_code");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn wrong_codes_pause_the_address(db: PgPool) {
    let app = TestApp::new(db);
    code_user(&app, "g@example.com").await;
    ask(&app, "g@example.com").await;
    let code = app.code_for("g@example.com", 1).await;
    let wrong = other_than(&code);

    for _ in 0..3 {
        assert_eq!(
            login(&app, "g@example.com", &wrong).await.code(),
            "invalid_code"
        );
    }
    let r = login(&app, "g@example.com", &code).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(r.body["error"]["details"]["retry_at"].is_string());

    // Wrong codes never use up the password's tries for the same address.
    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "g@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.code(), "invalid_credentials");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn guesses_without_a_live_code_count_nothing(db: PgPool) {
    let app = TestApp::new(db);
    code_user(&app, "h@example.com").await;
    for _ in 0..10 {
        assert_eq!(
            login(&app, "h@example.com", "000000").await.code(),
            "invalid_code"
        );
    }
    ask(&app, "h@example.com").await;
    let code = app.code_for("h@example.com", 1).await;
    assert_eq!(
        login(&app, "h@example.com", &code).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn one_mail_per_address_per_minute_and_the_newest_code_wins(db: PgPool) {
    let app = TestApp::new(db.clone());
    code_user(&app, "n@example.com").await;
    ask(&app, "n@example.com").await;
    ask(&app, "n@example.com").await;
    app.mail_to("n@example.com", 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(app.mail_to("n@example.com", 1).await.len(), 1);
    let first = app.code_for("n@example.com", 1).await;

    // A minute later a new code goes out, and the old one stops working.
    sqlx::query("UPDATE login_codes SET created_at = now() - interval '61 seconds'")
        .execute(&db)
        .await
        .unwrap();
    ask(&app, "n@example.com").await;
    let second = app.code_for("n@example.com", 2).await;
    if first != second {
        assert_eq!(
            login(&app, "n@example.com", &first).await.code(),
            "invalid_code"
        );
    }
    assert_eq!(
        login(&app, "n@example.com", &second).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_expired_code_does_not_work(db: PgPool) {
    let app = TestApp::new(db.clone());
    code_user(&app, "e@example.com").await;
    ask(&app, "e@example.com").await;
    let code = app.code_for("e@example.com", 1).await;
    sqlx::query("UPDATE login_codes SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        login(&app, "e@example.com", &code).await.code(),
        "invalid_code"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn one_ip_cannot_ask_for_endless_codes(db: PgPool) {
    let app = TestApp::with(db, "development", |s| s.login_code.max_requests_per_ip = 2);
    let ask_from = |email: &str| {
        Request::post("/api/auth/code")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.9")
            .body(Body::from(json!({ "email": email }).to_string()))
            .unwrap()
    };
    assert_eq!(
        app.send(ask_from("a@example.com")).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.send(ask_from("b@example.com")).await.status,
        StatusCode::NO_CONTENT
    );
    let r = app.send(ask_from("c@example.com")).await;
    assert_eq!(r.code(), "too_many_attempts");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn registering_with_a_code_proves_the_inbox(db: PgPool) {
    let app = TestApp::new(db);
    let body = |code: Option<&str>| {
        let mut b = json!({
            "method": "code", "email": "new@example.com", "first_name": "New", "last_name": "Person",
        });
        if let Some(code) = code {
            b["login_code"] = json!(code);
        }
        b
    };

    let r = app.post("/api/auth/register", body(None), None).await;
    assert_eq!(
        r.body["error"]["details"]["fields"]["login_code"],
        json!(["required"])
    );

    let r = app
        .post(
            "/api/auth/code",
            json!({ "email": "new@example.com", "purpose": "register" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let code = app.code_for("new@example.com", 1).await;

    // A login code is not a register code.
    let r = login(&app, "new@example.com", &code).await;
    assert_eq!(r.code(), "invalid_code");
    let r = app
        .post("/api/auth/register", body(Some(&other_than(&code))), None)
        .await;
    assert_eq!(r.code(), "invalid_code");

    let r = app
        .post("/api/auth/register", body(Some(&code)), None)
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "code");

    // Afterwards the account signs in with codes.
    ask(&app, "new@example.com").await;
    let code = app.code_for("new@example.com", 2).await;
    assert_eq!(
        login(&app, "new@example.com", &code).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_taken_email_is_said_only_after_a_right_code(db: PgPool) {
    let app = TestApp::new(db);
    app.register("taken@example.com").await;
    app.post(
        "/api/auth/code",
        json!({ "email": "taken@example.com", "purpose": "register" }),
        None,
    )
    .await;
    let code = app.code_for("taken@example.com", 1).await;
    let r = app
        .post(
            "/api/auth/register",
            json!({ "method": "code", "email": "taken@example.com", "first_name": "A", "last_name": "B", "login_code": code }),
            None,
        )
        .await;
    assert_eq!(r.code(), "email_taken");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn closed_registration_sends_no_register_codes(db: PgPool) {
    let app = TestApp::with(db, "development", |s| {
        s.registration.mode = app_core::config::RegistrationMode::Closed;
    });
    let r = app
        .post(
            "/api/auth/code",
            json!({ "email": "x@example.com", "purpose": "register" }),
            None,
        )
        .await;
    assert_eq!(r.code(), "registration_closed");
    let r = app
        .post(
            "/api/auth/code",
            json!({ "email": "x@example.com", "purpose": "change" }),
            None,
        )
        .await;
    assert_eq!(r.code(), "validation_failed");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_password_account_switches_to_codes(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (id, cookie) = app
        .user_with("s@example.com", Role::User, UserStatus::Active)
        .await;
    let other = app.login("s@example.com").await;

    let r = app
        .post("/api/me/login-code", json!({}), Some(&cookie))
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let code = app.code_for("s@example.com", 1).await;
    let mail = app.mail_to("s@example.com", 1).await;
    assert!(mail[0].subject.contains("change how you sign in"));

    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "code", "login_code": other_than(&code) }),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.code(), "invalid_code");
    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "code", "login_code": code }),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "code");
    assert_eq!(
        app.get("/api/me", Some(&other)).await.status,
        StatusCode::UNAUTHORIZED
    );

    // The password no longer works.
    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "s@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.code(), "invalid_credentials");
    let hash: Option<String> = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(hash, None);

    // Asking again for a method already in use is refused.
    let r = app
        .post("/api/me/login-code", json!({}), Some(&cookie))
        .await;
    assert_eq!(r.code(), "login_method_unchanged");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn asking_for_a_switch_code_needs_a_recent_sign_in(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (_, cookie) = app
        .user_with("o@example.com", Role::User, UserStatus::Active)
        .await;
    sqlx::query("UPDATE user_sessions SET created_at = now() - interval '11 minutes'")
        .execute(&db)
        .await
        .unwrap();
    let r = app
        .post("/api/me/login-code", json!({}), Some(&cookie))
        .await;
    assert_eq!(r.code(), "reauth_required");
    app.no_mail_to("o@example.com").await;
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_admin_gives_codes_at_creation_and_in_recovery(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;

    let r = app
        .post(
            "/api/users",
            json!({ "email": "made@example.com", "first_name": "M", "last_name": "U", "method": "code" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "code");
    ask(&app, "made@example.com").await;
    let code = app.code_for("made@example.com", 1).await;
    assert_eq!(
        login(&app, "made@example.com", &code).await.status,
        StatusCode::OK
    );

    let (id, _) = app
        .user_with("lost@example.com", Role::User, UserStatus::Active)
        .await;
    let r = app
        .put(
            &format!("/api/users/{id}/login-method"),
            json!({ "method": "code" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "code");
    let r = app
        .put(
            &format!("/api/users/{id}/login-method"),
            json!({ "method": "code" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.code(), "login_method_unchanged");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn health_says_where_mail_goes_and_codes_stay_out_of_answers(db: PgPool) {
    let app = TestApp::new(db.clone());
    assert_eq!(app.get("/api/health", None).await.body["mail"], "capture");

    code_user(&app, "q@example.com").await;
    ask(&app, "q@example.com").await;
    let code = app.code_for("q@example.com", 1).await;
    let stored: Vec<u8> = sqlx::query_scalar("SELECT code_hash FROM login_codes")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_ne!(stored, code.as_bytes());
    let r = login(&app, "q@example.com", &other_than(&code)).await;
    assert!(!r.body.to_string().contains(&code));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn parallel_guesses_get_only_the_codes_tries(db: PgPool) {
    let app = TestApp::new(db);
    code_user(&app, "p@example.com").await;
    assert_eq!(ask(&app, "p@example.com").await, StatusCode::NO_CONTENT);
    let code = app.code_for("p@example.com", 1).await;

    let wrong: Vec<String> = (1..=20)
        .map(|i| format!("{:06}", (code.parse::<u32>().unwrap() + i) % 1_000_000))
        .collect();
    let answers = common::join_all(
        wrong
            .iter()
            .map(|c| login(&app, "p@example.com", c))
            .collect(),
    )
    .await;
    for r in &answers {
        assert!(
            r.code() == "invalid_code" || r.code() == "too_many_attempts",
            "{}",
            r.body
        );
    }

    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM login_codes WHERE email = $1")
        .bind("p@example.com")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert!(attempts <= 3, "the code was tried {attempts} times");
    let failures: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_login_attempts WHERE email = $1 AND NOT success",
    )
    .bind("p@example.com")
    .fetch_one(&app.db)
    .await
    .unwrap();
    assert!(failures <= 3, "{failures} failures counted");
}
