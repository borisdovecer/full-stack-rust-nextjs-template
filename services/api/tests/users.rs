//! User management: list, detail, create, update, password, permissions, and their guards.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use app_api::auth::repository::{Role, UserStatus};
use common::{PASSWORD, TestApp};

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_plain_user_gets_forbidden_everywhere(db: PgPool) {
    let app = TestApp::new(db);
    let (admin_id, _) = app.admin().await;
    let (_, user) = app
        .user_with("u@example.com", Role::User, UserStatus::Active)
        .await;
    let id = admin_id.to_string();
    let calls = [
        app.get("/api/users", Some(&user)).await,
        app.get(&format!("/api/users/{id}"), Some(&user)).await,
        app.post(
            "/api/users",
            json!({ "email": "x@example.com", "first_name": "X", "last_name": "Y", "password": PASSWORD }),
            Some(&user),
        )
        .await,
        app.patch(
            &format!("/api/users/{id}"),
            json!({ "first_name": "X" }),
            Some(&user),
        )
        .await,
        app.put(
            &format!("/api/users/{id}/password"),
            json!({ "password": PASSWORD }),
            Some(&user),
        )
        .await,
        app.put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": [] }),
            Some(&user),
        )
        .await,
    ];
    for r in calls {
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.body);
        assert_eq!(r.code(), "forbidden");
    }
    assert_eq!(app.get("/api/users", None).await.code(), "unauthenticated");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn list_pages_filters_and_searches(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    for i in 0..5 {
        app.register(&format!("user{i}@example.com")).await;
    }
    let r = app
        .get("/api/users?per_page=2&page=2&sort=email_asc", Some(&admin))
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["total"], 6);
    assert_eq!(r.body["page"], 2);
    assert_eq!(r.body["items"].as_array().unwrap().len(), 2);
    assert_eq!(r.body["items"][0]["email"], "user1@example.com");

    let admins = app.get("/api/users?role=admin", Some(&admin)).await;
    assert_eq!(admins.body["total"], 1);
    let search = app.get("/api/users?search=USER3", Some(&admin)).await;
    assert_eq!(search.body["total"], 1);
    let bad = app.get("/api/users?status=gone", Some(&admin)).await;
    assert_eq!(bad.code(), "invalid_request");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn admin_creates_users_and_the_audit_shows_it(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let r = app
        .post(
            "/api/users",
            json!({ "email": "New@Example.com", "first_name": "New", "last_name": "User", "bio": " Ops ", "password": PASSWORD, "status": "pending" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert_eq!(r.body["user"]["email"], "new@example.com");
    assert_eq!(r.body["user"]["status"], "pending");
    assert_eq!(r.body["audit"][0]["action"], "user.created");
    assert_eq!(r.body["audit"][0]["actor_email"], "admin@example.com");

    let dup = app
        .post(
            "/api/users",
            json!({ "email": "new@example.com", "first_name": "New", "last_name": "User", "password": PASSWORD }),
            Some(&admin),
        )
        .await;
    assert_eq!(dup.code(), "email_taken");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn detail_shows_permissions_grants_and_404s(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, _) = app
        .user_with("u@example.com", Role::User, UserStatus::Active)
        .await;
    let r = app.get(&format!("/api/users/{id}"), Some(&admin)).await;
    assert_eq!(r.body["user"]["email"], "u@example.com");
    assert_eq!(r.body["permissions"], json!([]));
    assert_eq!(r.body["grants"], json!([]));

    let missing = app
        .get(
            "/api/users/00000000-0000-0000-0000-000000000000",
            Some(&admin),
        )
        .await;
    assert_eq!(missing.code(), "not_found");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn suspending_signs_the_user_out_and_is_audited(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, user) = app
        .user_with("u@example.com", Role::User, UserStatus::Active)
        .await;

    let r = app
        .patch(
            &format!("/api/users/{id}"),
            json!({ "status": "suspended" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["user"]["status"], "suspended");
    let event = &r.body["audit"][0];
    assert_eq!(event["action"], "user.updated");
    assert_eq!(event["data"]["changes"]["status"]["before"], "active");
    assert_eq!(event["data"]["changes"]["status"]["after"], "suspended");
    assert_eq!(event["data"]["sessions_revoked"], 1);

    assert_eq!(
        app.get("/api/me", Some(&user)).await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn approving_a_pending_user_lets_them_in(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, pending) = app
        .user_with("p@example.com", Role::User, UserStatus::Pending)
        .await;
    assert_eq!(
        app.get("/api/me/sessions", Some(&pending)).await.code(),
        "account_pending"
    );
    app.patch(
        &format!("/api/users/{id}"),
        json!({ "status": "active" }),
        Some(&admin),
    )
    .await;
    assert_eq!(
        app.get("/api/me/sessions", Some(&pending)).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn nobody_changes_their_own_role_status_or_permissions(db: PgPool) {
    let app = TestApp::new(db);
    let (id, admin) = app.admin().await;
    for body in [json!({ "role": "user" }), json!({ "status": "suspended" })] {
        let r = app
            .patch(&format!("/api/users/{id}"), body, Some(&admin))
            .await;
        assert_eq!(r.code(), "cannot_change_self");
    }
    let r = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": [] }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.code(), "cannot_change_self");

    // A name, or a role equal to the current one, is not a change of role.
    let r = app
        .patch(
            &format!("/api/users/{id}"),
            json!({ "first_name": "Boss", "last_name": "Person", "bio": "Runs things", "role": "admin" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["user"]["first_name"], "Boss");
    assert_eq!(r.body["user"]["last_name"], "Person");
    assert_eq!(r.body["user"]["bio"], "Runs things");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_manager_cannot_climb_to_admin(db: PgPool) {
    let app = TestApp::new(db);
    let (admin_id, admin) = app.admin().await;
    let (mgr_id, mgr) = app
        .user_with("m@example.com", Role::User, UserStatus::Active)
        .await;
    let (other_id, _) = app
        .user_with("o@example.com", Role::User, UserStatus::Active)
        .await;
    app.put(
        &format!("/api/users/{mgr_id}/permissions"),
        json!({ "permissions": ["users.read", "users.manage"] }),
        Some(&admin),
    )
    .await;

    // Manages plain users.
    let r = app
        .patch(
            &format!("/api/users/{other_id}"),
            json!({ "status": "suspended" }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);

    // Cannot make an admin, touch an admin, or create one.
    let r = app
        .patch(
            &format!("/api/users/{other_id}"),
            json!({ "role": "admin" }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
    let r = app
        .patch(
            &format!("/api/users/{admin_id}"),
            json!({ "status": "suspended" }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
    let r = app
        .patch(
            &format!("/api/users/{admin_id}"),
            json!({ "first_name": "Renamed", "bio": "changed by a manager" }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
    let r = app
        .put(
            &format!("/api/users/{admin_id}/password"),
            json!({ "password": PASSWORD }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
    let r = app
        .put(
            &format!("/api/users/{admin_id}/permissions"),
            json!({ "permissions": [] }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
    let r = app
        .post(
            "/api/users",
            json!({ "email": "a2@example.com", "first_name": "A2", "last_name": "B", "password": PASSWORD, "role": "admin" }),
            Some(&mgr),
        )
        .await;
    assert_eq!(r.code(), "forbidden");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn admins_can_change_other_admins(db: PgPool) {
    let app = TestApp::new(db);
    let (_, a) = app.admin().await;
    let (b_id, _) = app
        .user_with("b@example.com", Role::Admin, UserStatus::Active)
        .await;
    let r = app
        .patch(
            &format!("/api/users/{b_id}"),
            json!({ "role": "user" }),
            Some(&a),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["user"]["role"], "user");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn setting_a_password_signs_the_user_out(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, user) = app
        .user_with("u@example.com", Role::User, UserStatus::Active)
        .await;

    let weak = app
        .put(
            &format!("/api/users/{id}/password"),
            json!({ "password": "short" }),
            Some(&admin),
        )
        .await;
    assert_eq!(weak.code(), "validation_failed");

    let r = app
        .put(
            &format!("/api/users/{id}/password"),
            json!({ "password": "set-by-the-admin" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/me", Some(&user)).await.status,
        StatusCode::UNAUTHORIZED
    );
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": "u@example.com", "password": "set-by-the-admin" }),
            None,
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);

    let detail = app.get(&format!("/api/users/{id}"), Some(&admin)).await;
    assert_eq!(detail.body["audit"][0]["action"], "user.password_set");
    assert!(
        !detail.body["audit"]
            .to_string()
            .contains("set-by-the-admin")
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn grants_are_replaced_validated_and_audited(db: PgPool) {
    let app = TestApp::new(db);
    let (_, admin) = app.admin().await;
    let (id, user) = app
        .user_with("u@example.com", Role::User, UserStatus::Active)
        .await;

    let bad = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": ["users.read", "users.delete"] }),
            Some(&admin),
        )
        .await;
    assert_eq!(bad.code(), "validation_failed");
    assert_eq!(
        bad.body["error"]["details"]["fields"]["permissions"],
        json!(["unknown_permission"])
    );

    let r = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": ["users.read", "users.read"] }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.body["grants"][0]["permission"], "users.read");
    assert_eq!(r.body["grants"][0]["granted_by"], "admin@example.com");
    assert!(r.body["grants"][0]["granted_at"].is_string());
    assert_eq!(r.body["grants"].as_array().map(Vec::len), Some(1));
    assert_eq!(r.body["audit"][0]["action"], "user.permissions_changed");

    // Saving the same grants again changes nothing and records nothing, in any order.
    let changes = |body: &serde_json::Value| {
        body["audit"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["action"] == "user.permissions_changed")
            .count()
    };
    let both = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": ["users.read", "users.manage"] }),
            Some(&admin),
        )
        .await;
    let again = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": ["users.manage", "users.read"] }),
            Some(&admin),
        )
        .await;
    assert_eq!(changes(&again.body), changes(&both.body));
    let _ = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": ["users.read"] }),
            Some(&admin),
        )
        .await;
    assert_eq!(
        app.get("/api/users", Some(&user)).await.status,
        StatusCode::OK
    );

    let r = app
        .put(
            &format!("/api/users/{id}/permissions"),
            json!({ "permissions": [] }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.body["grants"], json!([]));
    assert_eq!(app.get("/api/users", Some(&user)).await.code(), "forbidden");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn nobody_sets_their_own_password_as_an_admin(db: PgPool) {
    let app = TestApp::new(db);
    let (id, admin) = app.admin().await;
    let r = app
        .put(
            &format!("/api/users/{id}/password"),
            json!({ "password": "another-long-password" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.code(), "cannot_change_self");
    assert_eq!(
        app.get("/api/me", Some(&admin)).await.status,
        StatusCode::OK
    );
}
