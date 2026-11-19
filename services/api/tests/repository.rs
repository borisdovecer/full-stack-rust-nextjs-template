//! Every repository function of `app-api` against a real database. Each test gets a fresh one.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{Duration, Utc};
use serde_json::json;
use sqlx::PgPool;

use app_api::auth::repository::models::{LoginType, Role, Theme, UserStatus};
use app_api::auth::repository::users::{ListUsers, LoginMethod, NewUser, UserPatch, UserSort};
use app_api::auth::repository::{
    login_attempts, login_codes, permissions, preferences, sessions, users,
};
use app_api::shared::repository::audit;

async fn user(
    pool: &PgPool,
    email: &str,
    name: &str,
    role: Role,
) -> app_api::auth::repository::User {
    users::create(
        pool,
        NewUser {
            email,
            first_name: name,
            last_name: "Test",
            bio: None,
            login_type: app_api::auth::repository::LoginType::Password,
            password_hash: Some("hash"),
            wallet: None,
            role,
            status: UserStatus::Active,
        },
    )
    .await
    .unwrap()
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn create_user_makes_preferences_and_refuses_duplicates(pool: PgPool) {
    let u = user(&pool, "ana@example.com", "Ana", Role::User).await;
    assert_eq!(u.status, UserStatus::Active);
    let prefs = preferences::get(&pool, u.id).await.unwrap();
    assert_eq!(prefs.theme, Theme::System);

    let err = users::create(
        &pool,
        NewUser {
            email: "ana@example.com",
            first_name: "Other",
            last_name: "Test",
            bio: None,
            login_type: app_api::auth::repository::LoginType::Password,
            password_hash: Some("h"),
            wallet: None,
            role: Role::User,
            status: UserStatus::Active,
        },
    )
    .await
    .unwrap_err();
    assert!(app_db::is_unique_violation(&err, users::EMAIL_KEY));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn database_refuses_unnormalised_email(pool: PgPool) {
    let err = users::create(
        &pool,
        NewUser {
            email: "Ana@Example.com",
            first_name: "Ana",
            last_name: "Test",
            bio: None,
            login_type: app_api::auth::repository::LoginType::Password,
            password_hash: Some("h"),
            wallet: None,
            role: Role::User,
            status: UserStatus::Active,
        },
    )
    .await;
    assert!(err.is_err());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn database_enforces_profile_and_sign_in_rules(pool: PgPool) {
    let u = user(&pool, "cy@example.com", "Cy", Role::User).await;
    let run = |sql: &'static str| {
        let pool = pool.clone();
        async move { sqlx::query(sql).bind(u.id).execute(&pool).await }
    };

    // A password account needs a password hash; a wallet account needs a wallet.
    assert!(
        run("UPDATE users SET password_hash = NULL WHERE id = $1")
            .await
            .is_err()
    );
    assert!(
        run("UPDATE users SET login_type = 'wallet' WHERE id = $1")
            .await
            .is_err()
    );
    // Code and external accounts need neither.
    run("UPDATE users SET login_type = 'code', password_hash = NULL WHERE id = $1")
        .await
        .unwrap();

    assert!(
        run("UPDATE users SET last_name = '  ' WHERE id = $1")
            .await
            .is_err()
    );
    assert!(
        run("UPDATE users SET bio = repeat('x', 1001) WHERE id = $1")
            .await
            .is_err()
    );
    run("UPDATE users SET bio = repeat('x', 1000) WHERE id = $1")
        .await
        .unwrap();

    // One wallet address belongs to one account.
    run("UPDATE users SET login_type = 'wallet', wallet = 'Addr1' WHERE id = $1")
        .await
        .unwrap();
    let other = user(&pool, "dee@example.com", "Dee", Role::User).await;
    let err = sqlx::query("UPDATE users SET wallet = 'Addr1' WHERE id = $1")
        .bind(other.id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(app_db::is_unique_violation(&err, "users_wallet_key"));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn find_update_and_credentials(pool: PgPool) {
    let u = user(&pool, "bo@example.com", "Bo", Role::User).await;
    assert_eq!(
        users::find(&pool, u.id).await.unwrap().unwrap().email,
        "bo@example.com"
    );
    assert!(
        users::find_by_email(&pool, "nobody@example.com")
            .await
            .unwrap()
            .is_none()
    );

    let creds = users::find_credentials_by_email(&pool, "bo@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(creds.password_hash.as_deref(), Some("hash"));
    assert!(!format!("{creds:?}").contains("hash\""));

    let updated = users::update(
        &pool,
        u.id,
        UserPatch {
            first_name: Some("Bob"),
            bio: Some(Some("Hello")),
            status: Some(UserStatus::Suspended),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.first_name, "Bob");
    assert_eq!(updated.last_name, "Test");
    assert_eq!(updated.bio.as_deref(), Some("Hello"));
    assert_eq!(
        updated.login_type,
        app_api::auth::repository::LoginType::Password
    );

    // Leaving the bio out keeps it; `Some(None)` clears it.
    let kept = users::update(&pool, u.id, UserPatch::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(kept.bio.as_deref(), Some("Hello"));
    let cleared = users::update(
        &pool,
        u.id,
        UserPatch {
            bio: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cleared.bio, None);
    assert_eq!(updated.role, Role::User);
    assert_eq!(updated.status, UserStatus::Suspended);
    assert!(updated.updated_at >= u.updated_at);

    assert!(users::set_password(&pool, u.id, "new").await.unwrap());
    users::touch_last_login(&pool, u.id).await.unwrap();
    let creds = users::find_credentials(&pool, u.id).await.unwrap().unwrap();
    assert_eq!(creds.password_hash.as_deref(), Some("new"));
    assert!(creds.user.last_login_at.is_some());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn list_counts_up_to_the_cap_and_stops_paging_there(pool: PgPool) {
    let cap = users::COUNT_CAP;
    sqlx::query(
        "INSERT INTO users (email, first_name, last_name, password_hash)
         SELECT 'u' || n || '@example.com', 'User', 'N' || n, 'hash'
         FROM generate_series(1, $1) AS n",
    )
    .bind(cap + 2)
    .execute(&pool)
    .await
    .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let page = |page: u32, search: Option<&str>| ListUsers {
        search: search.map(str::to_string),
        page,
        per_page: 20,
        ..Default::default()
    };

    let first = users::list(&mut conn, &page(1, None)).await.unwrap();
    assert_eq!(
        (first.items.len(), first.total, first.capped),
        (20, cap, true)
    );

    let last = users::list(&mut conn, &page(500, None)).await.unwrap();
    assert_eq!((last.items.len(), last.total, last.capped), (20, cap, true));

    let past = users::list(&mut conn, &page(501, None)).await.unwrap();
    assert_eq!((past.items.len(), past.total, past.capped), (0, cap, true));

    // Exactly the cap is an exact count.
    sqlx::query("DELETE FROM users WHERE email IN ('u1@example.com', 'u2@example.com')")
        .execute(&pool)
        .await
        .unwrap();
    let exact = users::list(&mut conn, &page(1, None)).await.unwrap();
    assert_eq!((exact.total, exact.capped), (cap, false));

    // A filter brings the total back under the cap.
    let few = users::list(&mut conn, &page(1, Some("u999@")))
        .await
        .unwrap();
    assert_eq!((few.items.len(), few.total, few.capped), (1, 1, false));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn list_filters_searches_sorts_and_pages(pool: PgPool) {
    user(&pool, "admin@example.com", "Zed Admin", Role::Admin).await;
    user(&pool, "a_b@example.com", "Ann", Role::User).await;
    user(&pool, "carl@example.com", "Carl", Role::User).await;

    let mut conn = pool.acquire().await.unwrap();
    let all = users::list(
        &mut conn,
        &ListUsers {
            sort: UserSort::NameAsc,
            page: 1,
            per_page: 2,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(all.total, 3);
    assert!(!all.capped);
    assert_eq!(
        all.items
            .iter()
            .map(|u| u.first_name.as_str())
            .collect::<Vec<_>>(),
        ["Ann", "Carl"]
    );

    // The short last page counts itself; a page past the end asks the database.
    for (n, len) in [(2, 1), (5, 0)] {
        let p = users::list(
            &mut conn,
            &ListUsers {
                page: n,
                per_page: 2,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!((p.items.len(), p.total), (len, 3), "page {n}");
    }

    let admins = users::list(
        &mut conn,
        &ListUsers {
            role: Some(Role::Admin),
            page: 1,
            per_page: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(admins.total, 1);

    // `_` is literal, not a wildcard: only a_b matches.
    let search = users::list(
        &mut conn,
        &ListUsers {
            search: Some("A_B".into()),
            page: 1,
            per_page: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(search.total, 1);

    // The search covers the full name, across the space between first and last.
    let full_name = users::list(
        &mut conn,
        &ListUsers {
            search: Some("carl test".into()),
            page: 1,
            per_page: 10,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(full_name.total, 1);

    let past_end = users::list(
        &mut conn,
        &ListUsers {
            page: 9,
            per_page: 2,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(past_end.items.is_empty());
    assert_eq!(past_end.total, 3);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn lock_active_admins_sees_only_active_admins(pool: PgPool) {
    let a = user(&pool, "a@example.com", "A", Role::Admin).await;
    let b = user(&pool, "b@example.com", "B", Role::Admin).await;
    user(&pool, "c@example.com", "C", Role::User).await;
    users::update(
        &pool,
        b.id,
        UserPatch {
            status: Some(UserStatus::Suspended),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        users::lock_active_admins(&mut tx).await.unwrap(),
        vec![a.id]
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn session_lifecycle(pool: PgPool) {
    let u = user(&pool, "s@example.com", "S", Role::User).await;
    let hash = [7u8; 32];
    let s = sessions::create(
        &pool,
        sessions::NewSession {
            user_id: u.id,
            token_hash: &hash,
            expires_at: Utc::now() + Duration::hours(1),
            ip: Some("127.0.0.1"),
            user_agent: Some("test"),
        },
    )
    .await
    .unwrap();

    let found = sessions::find_valid(&pool, &hash).await.unwrap().unwrap();
    assert_eq!(found.session_id, s.id);
    assert_eq!(found.user.id, u.id);
    assert!(
        sessions::find_valid(&pool, &[0u8; 32])
            .await
            .unwrap()
            .is_none()
    );

    let later = Utc::now() + Duration::hours(5);
    sessions::touch(&pool, s.id, Some(later)).await.unwrap();
    let found = sessions::find_valid(&pool, &hash).await.unwrap().unwrap();
    assert!(found.session_expires_at > Utc::now() + Duration::hours(4));

    let other = user(&pool, "o@example.com", "O", Role::User).await;
    assert!(!sessions::revoke(&pool, s.id, other.id).await.unwrap());
    assert_eq!(sessions::list_active(&pool, u.id).await.unwrap().len(), 1);
    assert!(sessions::revoke(&pool, s.id, u.id).await.unwrap());
    assert!(!sessions::revoke(&pool, s.id, u.id).await.unwrap());
    assert!(sessions::find_valid(&pool, &hash).await.unwrap().is_none());
    assert!(sessions::list_active(&pool, u.id).await.unwrap().is_empty());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn revoke_all_keeps_one_and_expired_sessions_are_invalid(pool: PgPool) {
    let u = user(&pool, "r@example.com", "R", Role::User).await;
    let mut ids = vec![];
    for i in 0..3u8 {
        let s = sessions::create(
            &pool,
            sessions::NewSession {
                user_id: u.id,
                token_hash: &[i; 32],
                expires_at: Utc::now() + Duration::hours(1),
                ip: None,
                user_agent: None,
            },
        )
        .await
        .unwrap();
        ids.push(s.id);
    }
    assert_eq!(
        sessions::revoke_all(&pool, u.id, Some(ids[0]))
            .await
            .unwrap(),
        2
    );
    assert!(
        sessions::find_valid(&pool, &[0u8; 32])
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(sessions::revoke_all(&pool, u.id, None).await.unwrap(), 1);

    sessions::create(
        &pool,
        sessions::NewSession {
            user_id: u.id,
            token_hash: &[9; 32],
            expires_at: Utc::now() - Duration::seconds(1),
            ip: None,
            user_agent: None,
        },
    )
    .await
    .unwrap();
    assert!(
        sessions::find_valid(&pool, &[9u8; 32])
            .await
            .unwrap()
            .is_none()
    );
    let t = Utc::now() + Duration::seconds(1);
    assert_eq!(sessions::delete_expired(&pool, t, 100).await.unwrap(), 1);
    // In batches: three revoked sessions, two at a time.
    assert_eq!(sessions::delete_revoked(&pool, t, 2).await.unwrap(), 2);
    assert_eq!(sessions::delete_revoked(&pool, t, 2).await.unwrap(), 1);
    assert_eq!(sessions::delete_revoked(&pool, t, 2).await.unwrap(), 0);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn login_failures_are_counted_per_method(pool: PgPool) {
    let since = Utc::now() - Duration::minutes(15);
    for _ in 0..3 {
        login_attempts::record(
            &pool,
            "y@example.com",
            LoginType::Code,
            Some("5.6.7.8"),
            false,
        )
        .await
        .unwrap();
    }
    let password =
        login_attempts::email_failures(&pool, "y@example.com", LoginType::Password, since)
            .await
            .unwrap();
    assert_eq!(password.count, 0);
    let code = login_attempts::email_failures(&pool, "y@example.com", LoginType::Code, since)
        .await
        .unwrap();
    assert_eq!(code.count, 3);

    // A password success does not clear the code failures; the IP still counts all of them.
    login_attempts::record(
        &pool,
        "y@example.com",
        LoginType::Password,
        Some("5.6.7.8"),
        true,
    )
    .await
    .unwrap();
    let code = login_attempts::email_failures(&pool, "y@example.com", LoginType::Code, since)
        .await
        .unwrap();
    assert_eq!(code.count, 3);
    let by_ip = login_attempts::ip_failures(&pool, "5.6.7.8", since)
        .await
        .unwrap();
    assert_eq!(by_ip.count, 3);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn setting_a_login_method_clears_the_other_credentials(pool: PgPool) {
    let u = user(&pool, "ed@example.com", "Ed", Role::User).await;

    let wallet = users::set_login_method(&pool, u.id, LoginMethod::Wallet { address: "Addr9" })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(wallet.login_type, LoginType::Wallet);
    assert_eq!(wallet.wallet.as_deref(), Some("Addr9"));
    let creds = users::find_credentials(&pool, u.id).await.unwrap().unwrap();
    assert_eq!(creds.password_hash, None);

    let code = users::set_login_method(&pool, u.id, LoginMethod::Code)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(code.login_type, LoginType::Code);
    assert_eq!(code.wallet, None);

    let back = users::set_login_method(&pool, u.id, LoginMethod::Password { hash: "h2" })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(back.login_type, LoginType::Password);
    let creds = users::find_credentials(&pool, u.id).await.unwrap().unwrap();
    assert_eq!(creds.password_hash.as_deref(), Some("h2"));

    assert!(
        users::set_login_method(&pool, uuid::Uuid::nil(), LoginMethod::Code)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn login_failures_reset_after_a_success(pool: PgPool) {
    let since = Utc::now() - Duration::minutes(15);
    for _ in 0..3 {
        login_attempts::record(
            &pool,
            "x@example.com",
            LoginType::Password,
            Some("1.2.3.4"),
            false,
        )
        .await
        .unwrap();
    }
    let f = login_attempts::email_failures(&pool, "x@example.com", LoginType::Password, since)
        .await
        .unwrap();
    assert_eq!(f.count, 3);
    assert!(f.oldest.is_some());

    login_attempts::record(
        &pool,
        "x@example.com",
        LoginType::Password,
        Some("1.2.3.4"),
        true,
    )
    .await
    .unwrap();
    login_attempts::record(
        &pool,
        "x@example.com",
        LoginType::Password,
        Some("1.2.3.4"),
        false,
    )
    .await
    .unwrap();
    let f = login_attempts::email_failures(&pool, "x@example.com", LoginType::Password, since)
        .await
        .unwrap();
    assert_eq!(f.count, 1);

    let by_ip = login_attempts::ip_failures(&pool, "1.2.3.4", since)
        .await
        .unwrap();
    assert_eq!(by_ip.count, 4);
    let t = Utc::now() + Duration::seconds(1);
    assert_eq!(login_attempts::delete_before(&pool, t, 3).await.unwrap(), 3);
    assert_eq!(login_attempts::delete_before(&pool, t, 3).await.unwrap(), 2);
    // Nothing newer than the cut-off goes.
    login_attempts::record(&pool, "x@example.com", LoginType::Password, None, false)
        .await
        .unwrap();
    let old = Utc::now() - Duration::days(1);
    assert_eq!(
        login_attempts::delete_before(&pool, old, 3).await.unwrap(),
        0
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_attempt_turns_into_a_success_or_goes(pool: PgPool) {
    let since = Utc::now() - Duration::minutes(15);
    let a = login_attempts::record(&pool, "z@example.com", LoginType::Password, None, false)
        .await
        .unwrap();
    let b = login_attempts::record(&pool, "z@example.com", LoginType::Password, None, false)
        .await
        .unwrap();
    let count = || async {
        login_attempts::email_failures(&pool, "z@example.com", LoginType::Password, since)
            .await
            .unwrap()
            .count
    };
    assert_eq!(count().await, 2);
    login_attempts::delete(&pool, a).await.unwrap();
    assert_eq!(count().await, 1);
    login_attempts::mark_success(&pool, b).await.unwrap();
    assert_eq!(count().await, 0);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn a_code_gives_exactly_its_tries_even_in_parallel(pool: PgPool) {
    let mut conn = pool.acquire().await.unwrap();
    let id = uuid::Uuid::new_v4();
    login_codes::create(
        &mut conn,
        login_codes::NewCode {
            id,
            email: "c@example.com",
            purpose: login_codes::CodePurpose::Login,
            code_hash: &[1; 32],
            ip: None,
            expires_at: Utc::now() + Duration::minutes(10),
        },
    )
    .await
    .unwrap();
    drop(conn);
    let tries = (0..20)
        .map(|_| login_codes::take_try(&pool, "c@example.com", login_codes::CodePurpose::Login, 3));
    let got = futures_join_all(tries).await;
    let taken = got
        .into_iter()
        .filter(|r| r.as_ref().unwrap().is_some())
        .count();
    assert_eq!(taken, 3);
    let none = login_codes::take_try(
        &pool,
        "c@example.com",
        login_codes::CodePurpose::Register,
        3,
    )
    .await
    .unwrap();
    assert!(none.is_none(), "a code only works for its purpose");
}

/// Polls every future together on this task, so their queries interleave.
async fn futures_join_all<F: std::future::Future>(futs: impl Iterator<Item = F>) -> Vec<F::Output> {
    let mut set = Vec::new();
    for f in futs {
        set.push(Box::pin(f));
    }
    let mut out: Vec<Option<F::Output>> = set.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut pending = false;
        for (i, f) in set.iter_mut().enumerate() {
            if out[i].is_none() {
                match f.as_mut().poll(cx) {
                    std::task::Poll::Ready(v) => out[i] = Some(v),
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

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn grants_name_who_gave_them(pool: PgPool) {
    let admin = user(&pool, "boss@example.com", "Boss", Role::Admin).await;
    let u = user(&pool, "g@example.com", "Gina", Role::User).await;
    let mut conn = pool.acquire().await.unwrap();
    permissions::replace(&mut conn, u.id, &["users.read".into()], Some(admin.id))
        .await
        .unwrap();
    let grants = permissions::list_grants(&pool, u.id).await.unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].permission, "users.read");
    assert_eq!(grants[0].granted_by.as_deref(), Some("boss@example.com"));
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn one_holder_of_an_advisory_lock(pool: PgPool) {
    let mut a = pool.acquire().await.unwrap();
    let mut b = pool.acquire().await.unwrap();
    assert!(app_db::try_advisory_lock(&mut a, 42).await.unwrap());
    assert!(!app_db::try_advisory_lock(&mut b, 42).await.unwrap());
    app_db::advisory_unlock(&mut a, 42).await.unwrap();
    assert!(app_db::try_advisory_lock(&mut b, 42).await.unwrap());
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn permissions_replace_and_preferences_update(pool: PgPool) {
    let admin = user(&pool, "ad@example.com", "Ad", Role::Admin).await;
    let u = user(&pool, "p@example.com", "P", Role::User).await;
    let mut tx = pool.begin().await.unwrap();
    permissions::replace(
        &mut tx,
        u.id,
        &["users.read".into(), "users.manage".into()],
        Some(admin.id),
    )
    .await
    .unwrap();
    permissions::replace(&mut tx, u.id, &["users.read".into()], Some(admin.id))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        permissions::list(&pool, u.id).await.unwrap(),
        ["users.read"]
    );

    let prefs = preferences::update(
        &pool,
        u.id,
        preferences::PreferencesPatch {
            theme: Some(Theme::Dark),
        },
    )
    .await
    .unwrap();
    assert_eq!(prefs.theme, Theme::Dark);
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn audit_events_are_listed_newest_first_with_actor(pool: PgPool) {
    let admin = user(&pool, "ad@example.com", "Ad", Role::Admin).await;
    let u = user(&pool, "q@example.com", "Q", Role::User).await;
    audit::record(
        &pool,
        None,
        Some(u.id),
        audit::action::USER_CREATED,
        json!({}),
    )
    .await
    .unwrap();
    audit::record(
        &pool,
        Some(admin.id),
        Some(u.id),
        audit::action::USER_UPDATED,
        json!({ "before": { "role": "user" }, "after": { "role": "admin" } }),
    )
    .await
    .unwrap();
    let events = audit::list_for_subject(&pool, u.id, 10).await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].action, audit::action::USER_UPDATED);
    assert_eq!(events[0].actor_email.as_deref(), Some("ad@example.com"));
    assert_eq!(events[1].actor_id, None);
}
