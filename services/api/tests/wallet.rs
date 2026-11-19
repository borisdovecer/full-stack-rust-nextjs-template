//! The `wallet` sign-in method: challenges, registering, signing in and switching with a
//! Solana wallet, and every way a signed message is refused.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sqlx::PgPool;

use app_auth::siws::{SiwsMessage, message_text};
use common::{PASSWORD, TestApp};

use app_api::auth::repository::{Role, UserStatus};

fn key(n: u8) -> SigningKey {
    SigningKey::from_bytes(&[n; 32])
}

fn address(k: &SigningKey) -> String {
    bs58::encode(k.verifying_key().as_bytes()).into_string()
}

/// What a browser sends after the wallet signed: address, message and signature.
struct Signed {
    address: String,
    message: String,
    signature: String,
}

impl Signed {
    fn fields(&self) -> Value {
        json!({ "address": self.address, "message": self.message, "signature": self.signature })
    }
}

/// Asks for a challenge for `k`'s address and signs it, letting `alter` change the message
/// first (a wrong domain, another nonce).
async fn sign_with(app: &TestApp, k: &SigningKey, alter: impl FnOnce(&mut SiwsMessage)) -> Signed {
    let addr = address(k);
    let r = app
        .post("/api/auth/wallet/nonce", json!({ "address": addr }), None)
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    let c = &r.body;
    let mut msg = SiwsMessage {
        domain: c["domain"].as_str().unwrap().into(),
        address: addr.clone(),
        statement: Some(c["statement"].as_str().unwrap().into()),
        nonce: Some(c["nonce"].as_str().unwrap().into()),
        issued_at: Some(c["issued_at"].as_str().unwrap().into()),
        expiration_time: Some(c["expiration_time"].as_str().unwrap().into()),
    };
    alter(&mut msg);
    let text = message_text(&msg);
    Signed {
        address: addr,
        message: STANDARD.encode(text.as_bytes()),
        signature: STANDARD.encode(k.sign(text.as_bytes()).to_bytes()),
    }
}

async fn sign(app: &TestApp, k: &SigningKey) -> Signed {
    sign_with(app, k, |_| {}).await
}

async fn register(app: &TestApp, email: &str, s: &Signed) -> common::Res {
    let mut body = s.fields();
    body["method"] = json!("wallet");
    body["email"] = json!(email);
    body["first_name"] = json!("Wally");
    body["last_name"] = json!("Wallet");
    app.post("/api/auth/register", body, None).await
}

async fn login(app: &TestApp, email: &str, s: &Signed) -> common::Res {
    let mut body = s.fields();
    body["method"] = json!("wallet");
    body["email"] = json!(email);
    app.post("/api/auth/login", body, None).await
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_wallet_registers_and_signs_in(db: PgPool) {
    let app = TestApp::new(db);
    let k = key(1);
    let r = register(&app, "w@example.com", &sign(&app, &k).await).await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "wallet");
    assert_eq!(r.body["user"]["wallet"], address(&k));

    let r = login(&app, "W@example.com", &sign(&app, &k).await).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert!(r.set_session.is_some());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn the_challenge_names_the_configured_domain_and_expires(db: PgPool) {
    let app = TestApp::new(db);
    let r = app
        .post(
            "/api/auth/wallet/nonce",
            json!({ "address": address(&key(1)) }),
            None,
        )
        .await;
    assert_eq!(r.body["domain"], "127.0.0.1:3000");
    assert!(
        r.body["statement"]
            .as_str()
            .unwrap()
            .contains("not a transaction")
    );
    assert!(r.body["nonce"].as_str().unwrap().len() >= 16);

    for bad in ["", "0xabc", "not base58 0OIl", "abc"] {
        let r = app
            .post("/api/auth/wallet/nonce", json!({ "address": bad }), None)
            .await;
        assert_eq!(
            r.body["error"]["details"]["fields"]["address"],
            json!(["invalid_address"]),
            "{bad}"
        );
    }
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn another_wallet_another_method_or_no_account_answer_the_same(db: PgPool) {
    let app = TestApp::new(db);
    register(&app, "w@example.com", &sign(&app, &key(1)).await).await;
    app.register("pw@example.com").await;

    let cases = [
        ("w@example.com", key(2)),
        ("pw@example.com", key(1)),
        ("nobody@example.com", key(1)),
    ];
    for (email, k) in cases {
        let r = login(&app, email, &sign(&app, &k).await).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{email}");
        assert_eq!(r.code(), "invalid_credentials", "{email}");
    }
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_challenge_works_once(db: PgPool) {
    let app = TestApp::new(db);
    let k = key(1);
    register(&app, "w@example.com", &sign(&app, &k).await).await;
    let s = sign(&app, &k).await;
    assert_eq!(
        login(&app, "w@example.com", &s).await.status,
        StatusCode::OK
    );
    assert_eq!(
        login(&app, "w@example.com", &s).await.code(),
        "invalid_credentials"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_bad_signature_still_spends_the_challenge(db: PgPool) {
    let app = TestApp::new(db);
    let k = key(1);
    register(&app, "w@example.com", &sign(&app, &k).await).await;
    let good = sign(&app, &k).await;
    let bad = Signed {
        signature: STANDARD.encode([0u8; 64]),
        ..Signed {
            address: good.address.clone(),
            message: good.message.clone(),
            signature: String::new(),
        }
    };
    assert_eq!(
        login(&app, "w@example.com", &bad).await.code(),
        "invalid_credentials"
    );
    assert_eq!(
        login(&app, "w@example.com", &good).await.code(),
        "invalid_credentials"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn messages_that_do_not_match_the_challenge_are_refused(db: PgPool) {
    let app = TestApp::new(db.clone());
    let k = key(1);
    register(&app, "w@example.com", &sign(&app, &k).await).await;

    let wrong_domain = sign_with(&app, &k, |m| m.domain = "evil.example".into()).await;
    assert_eq!(
        login(&app, "w@example.com", &wrong_domain).await.code(),
        "invalid_credentials"
    );
    let made_up_nonce = sign_with(&app, &k, |m| m.nonce = Some("MadeUpNonce12345".into())).await;
    assert_eq!(
        login(&app, "w@example.com", &made_up_nonce).await.code(),
        "invalid_credentials"
    );
    let stale = sign_with(&app, &k, |m| {
        m.issued_at = Some("2020-01-01T00:00:00Z".into())
    })
    .await;
    assert_eq!(
        login(&app, "w@example.com", &stale).await.code(),
        "invalid_credentials"
    );

    // A challenge issued for one address cannot be signed by another.
    let mine = sign(&app, &key(3)).await;
    let other = key(1);
    let text = String::from_utf8(STANDARD.decode(&mine.message).unwrap()).unwrap();
    let text = text.replace(&mine.address, &address(&other));
    let stolen = Signed {
        address: address(&other),
        message: STANDARD.encode(text.as_bytes()),
        signature: STANDARD.encode(other.sign(text.as_bytes()).to_bytes()),
    };
    assert_eq!(
        login(&app, "w@example.com", &stolen).await.code(),
        "invalid_credentials"
    );

    // An expired challenge.
    let late = sign(&app, &k).await;
    sqlx::query("UPDATE wallet_nonces SET expires_at = now() - interval '1 second'")
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        login(&app, "w@example.com", &late).await.code(),
        "invalid_credentials"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn registering_checks_the_proof_and_one_wallet_has_one_account(db: PgPool) {
    let app = TestApp::new(db);
    let r = app
        .post(
            "/api/auth/register",
            json!({ "method": "wallet", "email": "a@example.com", "first_name": "A", "last_name": "B", "address": "nope" }),
            None,
        )
        .await;
    let fields = &r.body["error"]["details"]["fields"];
    assert_eq!(fields["address"], json!(["invalid_address"]));
    assert_eq!(fields["message"], json!(["required"]));
    assert_eq!(fields["signature"], json!(["required"]));

    let k = key(1);
    let wrong = sign_with(&app, &k, |m| m.domain = "evil.example".into()).await;
    assert_eq!(
        register(&app, "a@example.com", &wrong).await.code(),
        "invalid_signature"
    );

    assert_eq!(
        register(&app, "a@example.com", &sign(&app, &k).await)
            .await
            .status,
        StatusCode::CREATED
    );
    let r = register(&app, "b@example.com", &sign(&app, &k).await).await;
    assert_eq!(r.status, StatusCode::CONFLICT);
    assert_eq!(r.code(), "wallet_taken");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_password_account_switches_to_a_wallet(db: PgPool) {
    let app = TestApp::new(db);
    let (id, cookie) = app
        .user_with("s@example.com", Role::User, UserStatus::Active)
        .await;
    let k = key(1);

    let mut body = sign_with(&app, &k, |m| m.domain = "evil.example".into())
        .await
        .fields();
    body["method"] = json!("wallet");
    let r = app.put("/api/me/login-method", body, Some(&cookie)).await;
    assert_eq!(r.code(), "invalid_signature");

    let mut body = sign(&app, &k).await.fields();
    body["method"] = json!("wallet");
    let r = app.put("/api/me/login-method", body, Some(&cookie)).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["login_type"], "wallet");
    assert_eq!(r.body["user"]["wallet"], address(&k));

    let r = app
        .post(
            "/api/auth/login",
            json!({ "email": "s@example.com", "password": PASSWORD }),
            None,
        )
        .await;
    assert_eq!(r.code(), "invalid_credentials");
    assert_eq!(
        login(&app, "s@example.com", &sign(&app, &k).await)
            .await
            .status,
        StatusCode::OK
    );

    // Someone else cannot switch to a wallet already in use.
    let (_, other) = app
        .user_with("t@example.com", Role::User, UserStatus::Active)
        .await;
    let mut body = sign(&app, &k).await.fields();
    body["method"] = json!("wallet");
    let r = app.put("/api/me/login-method", body, Some(&other)).await;
    assert_eq!(r.code(), "wallet_taken");

    let (_, admin) = app.admin().await;
    let detail = app.get(&format!("/api/users/{id}"), Some(&admin)).await;
    assert_eq!(
        detail.body["audit"][0]["action"],
        "user.login_method_changed"
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn one_ip_cannot_ask_for_endless_challenges(db: PgPool) {
    let app = TestApp::with(db, "development", |s| s.wallet.max_nonces_per_ip = 2);
    let ask = || {
        Request::post("/api/auth/wallet/nonce")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "198.51.100.11")
            .body(Body::from(
                json!({ "address": address(&key(1)) }).to_string(),
            ))
            .unwrap()
    };
    assert_eq!(app.send(ask()).await.status, StatusCode::OK);
    assert_eq!(app.send(ask()).await.status, StatusCode::OK);
    assert_eq!(app.send(ask()).await.code(), "too_many_attempts");
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_wallet_owner_who_lost_it_is_recovered_by_an_admin(db: PgPool) {
    let app = TestApp::new(db);
    let r = register(&app, "w@example.com", &sign(&app, &key(1)).await).await;
    let id = r.body["user"]["id"].as_str().unwrap().to_string();
    let (_, admin) = app.admin().await;
    let r = app
        .put(
            &format!("/api/users/{id}/login-method"),
            json!({ "method": "code" }),
            Some(&admin),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);
    assert_eq!(r.body["user"]["wallet"], Value::Null);
    // The old wallet no longer signs in, and is free for another account.
    assert_eq!(
        login(&app, "w@example.com", &sign(&app, &key(1)).await)
            .await
            .code(),
        "invalid_credentials"
    );
    assert_eq!(
        register(&app, "new@example.com", &sign(&app, &key(1)).await)
            .await
            .status,
        StatusCode::CREATED
    );
}
