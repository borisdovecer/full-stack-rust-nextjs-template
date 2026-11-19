//! A test client for the router: JSON requests, cookies, and quick ways to make users.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use app_core::{AppEnv, Config, Settings};

use app_api::auth::repository::users::NewUser;
use app_api::auth::repository::{Role, UserStatus};
use app_api::{app, state::AppState};

pub const PASSWORD: &str = "a-long-test-password";

pub struct TestApp {
    router: Router,
    pub db: PgPool,
    /// Mail the app "sent", newest last.
    pub outbox: app_mail::Outbox,
}

pub struct Res {
    pub status: StatusCode,
    pub body: Value,
    /// The session cookie's value from `Set-Cookie`; `Some("")` when it was cleared.
    pub set_session: Option<String>,
    pub headers: HeaderMap,
}

impl Res {
    pub fn code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }

    /// Where a redirect points.
    pub fn location(&self) -> &str {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }

    /// The value a `Set-Cookie` gives `name`; `Some("")` when it clears it.
    pub fn cookie(&self, name: &str) -> Option<String> {
        let prefix = format!("{name}=");
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|v| v.strip_prefix(prefix.as_str()))
            .map(|v| v.split(';').next().unwrap_or("").to_string())
    }
}

impl TestApp {
    pub fn new(db: PgPool) -> Self {
        Self::with(db, "development", |_| {})
    }

    pub fn with(db: PgPool, env: &str, tweak: impl FnOnce(&mut Settings)) -> Self {
        Self::with_config(db, env, |c| tweak(&mut c.settings))
    }

    /// Like `with`, for changes outside the settings file (secrets).
    pub fn with_config(db: PgPool, env: &str, tweak: impl FnOnce(&mut Config)) -> Self {
        let settings_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/app.toml");
        // Loaded as development, so the committed laptop addresses pass; production tests set
        // the environment afterwards. The production address checks are tested in app-core.
        let mut config = Config::from_lookup(|k| match k {
            "DATABASE_URL" => Some("postgres://unused@127.0.0.1/unused".into()),
            "APP_CONFIG" => Some(settings_path.into()),
            _ => None,
        })
        .unwrap();
        config.env = match env {
            "production" => AppEnv::Production,
            _ => AppEnv::Development,
        };
        tweak(&mut config);
        config.settings.validate().unwrap();
        let (mailer, outbox) = app_mail::Mailer::capture();
        Self {
            router: app::router(AppState::with_mailer(config, db.clone(), mailer)),
            db,
            outbox,
        }
    }

    pub async fn call(
        &self,
        method: Method,
        uri: &str,
        body: Option<Value>,
        cookie: Option<&str>,
    ) -> Res {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(c) = cookie {
            req = req.header(header::COOKIE, format!("app_session={c}"));
        }
        let req = match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        self.send(req).await
    }

    pub async fn send(&self, req: Request<Body>) -> Res {
        let res = self.router.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        let set_session = res
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|v| v.strip_prefix("app_session="))
            .map(|v| v.split(';').next().unwrap_or("").to_string());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        Res {
            status,
            body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            set_session,
            headers,
        }
    }

    pub async fn get(&self, uri: &str, cookie: Option<&str>) -> Res {
        self.call(Method::GET, uri, None, cookie).await
    }

    pub async fn post(&self, uri: &str, body: Value, cookie: Option<&str>) -> Res {
        self.call(Method::POST, uri, Some(body), cookie).await
    }

    pub async fn patch(&self, uri: &str, body: Value, cookie: Option<&str>) -> Res {
        self.call(Method::PATCH, uri, Some(body), cookie).await
    }

    pub async fn put(&self, uri: &str, body: Value, cookie: Option<&str>) -> Res {
        self.call(Method::PUT, uri, Some(body), cookie).await
    }

    pub async fn delete(&self, uri: &str, cookie: Option<&str>) -> Res {
        self.call(Method::DELETE, uri, Some(json!({})), cookie)
            .await
    }

    /// Registers through the API and returns the session cookie.
    pub async fn register(&self, email: &str) -> String {
        let r = self
            .post(
                "/api/auth/register",
                json!({ "email": email, "first_name": "Test", "last_name": "User", "password": PASSWORD }),
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
        r.set_session.unwrap()
    }

    pub async fn login(&self, email: &str) -> String {
        let r = self
            .post(
                "/api/auth/login",
                json!({ "email": email, "password": PASSWORD }),
                None,
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.body);
        r.set_session.unwrap()
    }

    /// Inserts a user straight into the database and signs them in.
    pub async fn user_with(
        &self,
        email: &str,
        role: Role,
        status: UserStatus,
    ) -> (uuid::Uuid, String) {
        let hash = app_auth::password::hash(PASSWORD).unwrap();
        let user = app_api::auth::repository::users::create(
            &self.db,
            NewUser {
                email,
                first_name: "Test",
                last_name: "User",
                bio: None,
                login_type: app_api::auth::repository::LoginType::Password,
                password_hash: Some(&hash),
                wallet: None,
                role,
                status,
            },
        )
        .await
        .unwrap();
        let cookie = if status == UserStatus::Suspended {
            String::new()
        } else {
            self.login(email).await
        };
        (user.id, cookie)
    }

    /// Switches an account to another method straight in the database, as parts not built
    /// yet would, so the routes can be tried against it.
    pub async fn set_login_type(
        &self,
        id: uuid::Uuid,
        method: app_api::auth::repository::users::LoginMethod<'_>,
    ) {
        app_api::auth::repository::users::set_login_method(&self.db, id, method)
            .await
            .unwrap()
            .unwrap();
    }

    /// Mail sent to `to` so far. Mail leaves in the background, so this waits a little for it.
    pub async fn mail_to(&self, to: &str, at_least: usize) -> Vec<app_mail::Message> {
        for _ in 0..100 {
            let mail: Vec<_> = self
                .outbox
                .lock()
                .unwrap()
                .iter()
                .filter(|m| m.to == to)
                .cloned()
                .collect();
            if mail.len() >= at_least {
                return mail;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("expected {at_least} mail(s) to {to}");
    }

    /// The code in the newest mail to `to`, waiting for mail number `nth` (from 1).
    pub async fn code_for(&self, to: &str, nth: usize) -> String {
        let mail = self.mail_to(to, nth).await;
        let text = &mail[nth - 1].text;
        text.split_whitespace()
            .find(|w| w.len() == 6 && w.bytes().all(|b| b.is_ascii_digit()))
            .unwrap()
            .to_string()
    }

    /// Nothing was mailed to `to`, after giving background sends a moment.
    pub async fn no_mail_to(&self, to: &str) {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !self.outbox.lock().unwrap().iter().any(|m| m.to == to),
            "unexpected mail to {to}"
        );
    }

    pub async fn admin(&self) -> (uuid::Uuid, String) {
        self.user_with("admin@example.com", Role::Admin, UserStatus::Active)
            .await
    }
}

/// Runs every future together on this task, so their requests interleave as parallel requests
/// to a server would.
pub async fn join_all<F: std::future::Future>(futs: Vec<F>) -> Vec<F::Output> {
    let mut futs: Vec<_> = futs.into_iter().map(Box::pin).collect();
    let mut out: Vec<Option<F::Output>> = futs.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (f, slot) in futs.iter_mut().zip(out.iter_mut()) {
            if slot.is_none() {
                match f.as_mut().poll(cx) {
                    std::task::Poll::Ready(v) => *slot = Some(v),
                    std::task::Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    })
    .await;
    out.into_iter().map(|o| o.unwrap()).collect()
}
