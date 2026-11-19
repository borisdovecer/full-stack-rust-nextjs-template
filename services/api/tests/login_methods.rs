//! Sign-in methods: what the server offers, logging in with the wrong method, per-method
//! throttling, and changing a method (by the person and by an admin).
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use app_api::auth::repository::users::LoginMethod;
use app_api::auth::repository::{LoginType, Role, UserStatus, login_attempts};
use common::{PASSWORD, TestApp};

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn config_lists_the_available_methods(db: PgPool) {
    let app = TestApp::new(db);
    let r = app.get("/api/config", None).await;
    assert_eq!(
        r.body["login_methods"],
        json!(["password", "code", "wallet"])
    );
    assert_eq!(r.body["admin_login_methods"], json!(["password", "code"]));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn codes_need_real_mail_in_production(db: PgPool) {
    // The test mailer delivers nothing, like a server without RESEND_API_KEY.
    let app = TestApp::with(db, "production", |_| {});
    let r = app.get("/api/config", None).await;
    assert_eq!(r.body["login_methods"], json!(["password", "wallet"]));
    let r = app
        .post("/api/auth/code", json!({ "email": "a@example.com" }), None)
        .await;
    assert_eq!(r.code(), "login_method_unavailable");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_method_that_is_not_built_is_refused(db: PgPool) {
    let app = TestApp::new(db);
    for method in ["external"] {
        let r = app
            .post(
                "/api/auth/login",
                json!({ "method": method, "email": "a@example.com" }),
                None,
            )
            .await;
        assert_eq!(r.code(), "login_method_unavailable", "{method}");
        let r = app
            .post(
                "/api/auth/register",
                json!({ "method": method, "email": "a@example.com", "first_name": "A", "last_name": "B" }),
                None,
            )
            .await;
        assert_eq!(r.code(), "login_method_unavailable", "{method}");
    }
    let r = app
        .post(
            "/api/auth/login",
            json!({ "method": "carrier-pigeon", "email": "a@example.com" }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn register_and_login_take_an_explicit_password_method(db: PgPool) {
    let app = TestApp::new(db);
    let r = app
        .post(
            "/api/auth/register",
            json!({ "method": "password", "email": "a@example.com", "first_name": "A", "last_name": "B" }),
            None,
        )
        .await;
    assert_eq!(r.code(), "validation_failed");
    assert_eq!(
        r.body["error"]["details"]["fields"]["password"],
        json!(["required"])
    );

    let r = app
        .post(
            "/api/auth/register",
            json!({ "method": "password", "email": "a@example.com", "first_name": "A", "last_name": "B", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["user"]["login_type"], "password");

    let r = app
        .post(
            "/api/auth/login",
            json!({ "method": "password", "email": "a@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_account_with_another_method_answers_like_a_wrong_password(db: PgPool) {
    let app = TestApp::new(db);
    let (id, _) = app
        .user_with("c@example.com", Role::User, UserStatus::Active)
        .await;
    app.set_login_type(id, LoginMethod::Code).await;

    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "c@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.code(), "invalid_credentials");
    let unknown = app
        .post(
            "/api/auth/login",
            json!({ "email": "nobody@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(unknown.body["error"]["code"], r.body["error"]["code"]);
    assert_eq!(unknown.body["error"]["message"], r.body["error"]["message"]);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn failures_with_another_method_do_not_throttle_the_password(db: PgPool) {
    let app = TestApp::new(db.clone());
    app.register("t@example.com").await;
    for _ in 0..10 {
        login_attempts::record(&db, "t@example.com", LoginType::Code, None, false)
            .await
            .unwrap();
    }
    app.login("t@example.com").await;
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_person_switches_to_a_password_and_other_sessions_end(db: PgPool) {
    let app = TestApp::new(db);
    let (id, first) = app
        .user_with("s@example.com", Role::User, UserStatus::Active)
        .await;
    let second = app.login("s@example.com").await;

    // Already a password account: a new password goes through /api/me/password.
    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "password", "password": "another-password-1" }),
            Some(&first),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.code(), "login_method_unchanged");

    app.set_login_type(id, LoginMethod::Code).await;
    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "password" }),
            Some(&first),
        )
        .await;
    assert_eq!(r.code(), "validation_failed");
    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "external" }),
            Some(&first),
        )
        .await;
    assert_eq!(r.code(), "login_method_unavailable");

    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "password", "password": "another-password-1" }),
            Some(&first),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "password");
    assert_eq!(
        app.get("/api/me", Some(&first)).await.status,
        StatusCode::OK
    );
    assert_eq!(
        app.get("/api/me", Some(&second)).await.status,
        StatusCode::UNAUTHORIZED
    );
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": "s@example.com", "password": "another-password-1" }),
            None,
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);

    let (_, admin) = app.admin().await;
    let detail = app.get(&format!("/api/users/{id}"), Some(&admin)).await;
    assert_eq!(
        detail.body["audit"][0]["action"],
        "user.login_method_changed"
    );
    assert_eq!(
        detail.body["audit"][0]["data"]["login_type"],
        json!({ "before": "code", "after": "password" })
    );
    assert!(
        !detail.body["audit"]
            .to_string()
            .contains("another-password-1")
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn changing_the_method_needs_a_recent_sign_in(db: PgPool) {
    let app = TestApp::new(db.clone());
    let (id, cookie) = app
        .user_with("r@example.com", Role::User, UserStatus::Active)
        .await;
    app.set_login_type(id, LoginMethod::Code).await;
    sqlx::query("UPDATE user_sessions SET created_at = now() - interval '11 minutes'")
        .execute(&db)
        .await
        .unwrap();

    let r = app
        .put(
            "/api/me/login-method",
            json!({ "method": "password", "password": "another-password-1" }),
            Some(&cookie),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert_eq!(r.code(), "reauth_required");
    // Nothing changed.
    assert_eq!(
        app.get("/api/me", Some(&cookie)).await.body["user"]["login_type"],
        "code"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_admin_recovers_an_account_with_a_password(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, user) = app
        .user_with("w@example.com", Role::User, UserStatus::Active)
        .await;
    app.set_login_type(
        id,
        LoginMethod::Wallet {
            address: "LostWallet1",
        },
    )
    .await;
    let uri = format!("/api/users/{id}/login-method");

    let r = app
        .put(&uri, json!({ "method": "wallet" }), Some(&admin))
        .await;
    assert_eq!(r.code(), "login_method_unavailable");
    let r = app
        .put(&uri, json!({ "method": "password" }), Some(&admin))
        .await;
    assert_eq!(r.code(), "validation_failed");

    let r = app
        .put(
            &uri,
            json!({ "method": "password", "password": "recovered-password" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "password");
    assert_eq!(r.body["user"]["wallet"], serde_json::Value::Null);
    assert_eq!(r.body["audit"][0]["action"], "user.login_method_changed");
    assert_eq!(
        app.get("/api/me", Some(&user)).await.status,
        StatusCode::UNAUTHORIZED
    );
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": "w@example.com", "password": "recovered-password" }),
            None,
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn recovery_keeps_the_admin_rules(db: PgPool) {
    let app = TestApp::new(db);
    let (admin_id, admin) = app.admin().await;
    let body = json!({ "method": "password", "password": "recovered-password" });

    let r = app
        .put(
            &format!("/api/users/{admin_id}/login-method"),
            body.clone(),
            Some(&admin),
        )
        .await;
    assert_eq!(r.code(), "cannot_change_self");

    let (manager_id, manager) = app
        .user_with("m@example.com", Role::User, UserStatus::Active)
        .await;
    let r = app
        .put(
            &format!("/api/users/{manager_id}/permissions"),
            json!({ "permissions": ["users.read", "users.manage"] }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    let r = app
        .put(
            &format!("/api/users/{admin_id}/login-method"),
            body.clone(),
            Some(&manager),
        )
        .await;
    assert_eq!(r.code(), "forbidden");

    let (plain_id, plain) = app
        .user_with("p@example.com", Role::User, UserStatus::Active)
        .await;
    let r = app
        .put(
            &format!("/api/users/{plain_id}/login-method"),
            body,
            Some(&plain),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn setting_a_password_makes_it_the_method(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, _) = app
        .user_with("x@example.com", Role::User, UserStatus::Active)
        .await;
    app.set_login_type(id, LoginMethod::Code).await;

    let r = app
        .put(
            &format!("/api/users/{id}/password"),
            json!({ "password": "set-by-the-admin" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    let detail = app.get(&format!("/api/users/{id}"), Some(&admin)).await;
    assert_eq!(detail.body["user"]["login_type"], "password");
    assert_eq!(
        detail.body["audit"][0]["data"]["login_type"],
        json!({ "before": "code", "after": "password" })
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn admins_create_only_methods_they_may_give(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let base = json!({ "email": "n@example.com", "first_name": "N", "last_name": "U" });

    let mut wallet = base.clone();
    wallet["method"] = json!("wallet");
    let r = app.post("/api/users", wallet, Some(&admin)).await;
    assert_eq!(r.code(), "login_method_unavailable");

    let r = app.post("/api/users", base.clone(), Some(&admin)).await;
    assert_eq!(
        r.body["error"]["details"]["fields"]["password"],
        json!(["required"])
    );

    let mut ok = base;
    ok["password"] = json!(PASSWORD);
    let r = app.post("/api/users", ok, Some(&admin)).await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["user"]["login_type"], "password");
}
