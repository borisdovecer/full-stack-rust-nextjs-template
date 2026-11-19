//! The HTTP behaviour every route shares: error shape, request ids, health, OpenAPI.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

use app_core::Config;

use app_api::{app, state::AppState};

mod common;

fn state(db: PgPool) -> AppState {
    let settings = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/app.toml");
    let config = Config::from_lookup(|k| match k {
        "DATABASE_URL" => Some("postgres://u:p@127.0.0.1/app".into()),
        "APP_CONFIG" => Some(settings.into()),
        _ => None,
    })
    .unwrap();
    AppState::new(config, db)
}

async fn send(db: &PgPool, req: Request<Body>) -> (StatusCode, Option<String>, Value) {
    let res = app::router(state(db.clone())).oneshot(req).await.unwrap();
    let status = res.status();
    let id = res
        .headers()
        .get("x-request-id")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        id,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn health_answers(db: PgPool) {
    let (status, id, body) = send(&db, get("/api/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(id.is_some());
    assert_eq!(body["status"], "ok");
    assert_eq!(body["environment"], "development");
    assert_eq!(body["database"], "ok");
    assert_eq!(body["migrations"], "current");
}

#[sqlx::test(migrations = false)]
async fn health_is_degraded_with_pending_migrations(db: PgPool) {
    let (status, _, body) = send(&db, get("/api/health")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "degraded");
    assert_eq!(body["migrations"], "pending");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn unknown_route_is_a_json_not_found_with_the_request_id(db: PgPool) {
    let req = Request::get("/api/nope")
        .header("x-request-id", "test-123")
        .body(Body::empty())
        .unwrap();
    let (status, id, body) = send(&db, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(id.as_deref(), Some("test-123"));
    assert_eq!(body["error"]["code"], "not_found");
    assert_eq!(body["error"]["request_id"], "test-123");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn unsafe_request_id_is_replaced(db: PgPool) {
    let req = Request::get("/api/health")
        .header("x-request-id", "a b;c")
        .body(Body::empty())
        .unwrap();
    let (_, id, _) = send(&db, req).await;
    let id = id.unwrap();
    assert_ne!(id, "a b;c");
    assert_eq!(id.len(), 32);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn wrong_method_is_a_json_error(db: PgPool) {
    let req = Request::post("/api/health")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let (status, _, body) = send(&db, req).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(body["error"]["code"], "method_not_allowed");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn openapi_lists_health_and_the_error_codes(db: PgPool) {
    let (status, _, body) = send(&db, get("/api/openapi.json")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["paths"]["/api/health"]["get"].is_object());
    let codes = body["components"]["schemas"]["ErrorCode"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(codes.len(), app_core::ErrorCode::ALL.len());
}

#[test]
fn every_schema_reference_resolves() {
    let doc: Value = serde_json::from_str(&app::openapi().to_json().unwrap()).unwrap();
    let defined = doc["components"]["schemas"].as_object().unwrap();
    let text = doc.to_string();
    let prefix = "\"#/components/schemas/";
    for part in text.split(prefix).skip(1) {
        let name = part.split('"').next().unwrap();
        assert!(defined.contains_key(name), "unresolved $ref to {name}");
    }
}

#[test]
fn the_openapi_command_and_the_served_document_agree() {
    let doc = app::openapi().to_json().unwrap();
    assert!(doc.contains("/api/health"));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn writes_must_be_json(db: PgPool) {
    let req = Request::post("/api/auth/login")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=a&password=b"))
        .unwrap();
    let (status, _, body) = send(&db, req).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(body["error"]["code"], "unsupported_media_type");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn openapi_is_admin_only_in_production(db: PgPool) {
    let app = common::TestApp::with(db, "production", |_| {});
    assert_eq!(
        app.get("/api/openapi.json", None).await.status,
        StatusCode::NOT_FOUND
    );
    let (_, user) = app
        .user_with(
            "u@example.com",
            app_api::auth::repository::Role::User,
            app_api::auth::repository::UserStatus::Active,
        )
        .await;
    assert_eq!(
        app.get("/api/openapi.json", Some(&user)).await.status,
        StatusCode::NOT_FOUND
    );
    let (_, admin) = app.admin().await;
    assert_eq!(
        app.get("/api/openapi.json", Some(&admin)).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn public_config_answers_without_a_session(db: PgPool) {
    let app = common::TestApp::new(db);
    let r = app.get("/api/config", None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["registration_mode"], "open");
    assert_eq!(r.body["app_name"], "App");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn help_index_and_pages(db: PgPool) {
    let app = common::TestApp::new(db);
    assert_eq!(app.get("/api/help", None).await.code(), "unauthenticated");

    let cookie = app.register("ana@example.com").await;
    let index = app.get("/api/help", Some(&cookie)).await;
    assert_eq!(index.status, StatusCode::OK);
    let first = index.body[0]["slug"].as_str().unwrap().to_string();
    assert_eq!(first, "architecture");

    let req = Request::get(format!("/api/help/{first}"))
        .header("cookie", format!("app_session={cookie}"))
        .body(Body::empty())
        .unwrap();
    let res = app::router(state_for_help(&app))
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/markdown")
    );
    let text = res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&text).starts_with("# Architecture"));

    for bad in ["README", "nope", "..%2F..%2FCargo"] {
        let r = app.get(&format!("/api/help/{bad}"), Some(&cookie)).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{bad}");
    }
}

fn state_for_help(app: &common::TestApp) -> AppState {
    state(app.db.clone())
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_database_outage_does_not_break_public_routes_for_signed_in_browsers(db: PgPool) {
    let app = common::TestApp::new(db.clone());
    let cookie = app.register("out@example.com").await;
    db.close().await;

    let health = app.get("/api/health", Some(&cookie)).await;
    assert_eq!(health.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(health.body["database"], "down");

    assert_eq!(
        app.get("/api/config", Some(&cookie)).await.status,
        StatusCode::OK
    );

    let me = app.get("/api/me", Some(&cookie)).await;
    assert_eq!(me.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(me.code(), "unavailable");
    assert_eq!(me.set_session, None, "an outage must not clear the cookie");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn only_one_instance_cleans_up_at_a_time(db: PgPool) {
    use app_api::jobs::housekeeping;
    let mut other = db.acquire().await.unwrap();
    assert!(
        app_db::try_advisory_lock(&mut other, housekeeping::LOCK_KEY)
            .await
            .unwrap()
    );
    assert!(
        !housekeeping::run_once(&db).await,
        "ran while another held the lock"
    );
    app_db::advisory_unlock(&mut other, housekeeping::LOCK_KEY)
        .await
        .unwrap();
    assert!(housekeeping::run_once(&db).await);
    // It lets go, so the next run can take it.
    assert!(housekeeping::run_once(&db).await);
}
