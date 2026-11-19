//! The `users` table. Emails must already be normalised ([`crate::shared::validation::normalize_email`]).

use sqlx::{PgConnection, PgExecutor, QueryBuilder};
use uuid::Uuid;

use super::models::{Credentials, LoginType, Role, User, UserStatus};

/// The constraint whose violation means "email taken".
pub const EMAIL_KEY: &str = "users_email_key";
/// The index whose violation means "wallet taken".
pub const WALLET_KEY: &str = "users_wallet_key";

/// The `User` columns, optionally prefixed with a table alias. A macro so the result is a
/// literal, which `sqlx::query` accepts as safe SQL.
macro_rules! user_columns {
    () => {
        "id, email, first_name, last_name, bio, login_type, wallet, role, status, last_login_at, \
         created_at, updated_at"
    };
    ($p:literal) => {
        concat!(
            $p,
            ".id, ",
            $p,
            ".email, ",
            $p,
            ".first_name, ",
            $p,
            ".last_name, ",
            $p,
            ".bio, ",
            $p,
            ".login_type, ",
            $p,
            ".wallet, ",
            $p,
            ".role, ",
            $p,
            ".status, ",
            $p,
            ".last_login_at, ",
            $p,
            ".created_at, ",
            $p,
            ".updated_at"
        )
    };
}
pub(crate) use user_columns;

#[derive(Debug, Clone)]
pub struct NewUser<'a> {
    pub email: &'a str,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub bio: Option<&'a str>,
    pub login_type: LoginType,
    /// Required for `LoginType::Password`, enforced by `users_login_type_credentials`.
    pub password_hash: Option<&'a str>,
    /// Required for `LoginType::Wallet`; unique (`WALLET_KEY`).
    pub wallet: Option<&'a str>,
    pub role: Role,
    pub status: UserStatus,
}

/// Creates the user and its preferences row in one statement.
pub async fn create<'e>(ex: impl PgExecutor<'e>, new: NewUser<'_>) -> Result<User, sqlx::Error> {
    sqlx::query_as(concat!(
        "WITH u AS (
            INSERT INTO users
                (email, first_name, last_name, bio, login_type, password_hash, wallet, role, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING ",
        user_columns!(),
        "
        ), p AS (
            INSERT INTO user_preferences (user_id) SELECT id FROM u
        )
        SELECT * FROM u"
    ))
    .bind(new.email)
    .bind(new.first_name)
    .bind(new.last_name)
    .bind(new.bio)
    .bind(new.login_type)
    .bind(new.password_hash)
    .bind(new.wallet)
    .bind(new.role)
    .bind(new.status)
    .fetch_one(ex)
    .await
}

pub async fn find<'e>(ex: impl PgExecutor<'e>, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        " FROM users WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(ex)
    .await
}

pub async fn find_by_email<'e>(
    ex: impl PgExecutor<'e>,
    email: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        " FROM users WHERE email = $1"
    ))
    .bind(email)
    .fetch_optional(ex)
    .await
}

pub async fn find_credentials_by_email<'e>(
    ex: impl PgExecutor<'e>,
    email: &str,
) -> Result<Option<Credentials>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        ", password_hash FROM users WHERE email = $1"
    ))
    .bind(email)
    .fetch_optional(ex)
    .await
}

pub async fn find_credentials<'e>(
    ex: impl PgExecutor<'e>,
    id: Uuid,
) -> Result<Option<Credentials>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        user_columns!(),
        ", password_hash FROM users WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(ex)
    .await
}

/// The fields an update may change. `None` leaves a field as it is.
#[derive(Debug, Clone, Default)]
pub struct UserPatch<'a> {
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    /// `Some(None)` clears the bio.
    pub bio: Option<Option<&'a str>>,
    pub role: Option<Role>,
    pub status: Option<UserStatus>,
}

pub async fn update<'e>(
    ex: impl PgExecutor<'e>,
    id: Uuid,
    patch: UserPatch<'_>,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as(concat!(
        "UPDATE users SET
            first_name = COALESCE($2, first_name),
            last_name = COALESCE($3, last_name),
            bio = CASE WHEN $4 THEN $5 ELSE bio END,
            role = COALESCE($6, role),
            status = COALESCE($7, status)
         WHERE id = $1
         RETURNING ",
        user_columns!()
    ))
    .bind(id)
    .bind(patch.first_name)
    .bind(patch.last_name)
    .bind(patch.bio.is_some())
    .bind(patch.bio.flatten())
    .bind(patch.role)
    .bind(patch.status)
    .fetch_optional(ex)
    .await
}

pub async fn set_password<'e>(
    ex: impl PgExecutor<'e>,
    id: Uuid,
    password_hash: &str,
) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(password_hash)
        .execute(ex)
        .await?;
    Ok(r.rows_affected() == 1)
}

/// How an account signs in from now on. Sets the method and its credential, and clears the
/// credentials of the other methods (including a tied Google or GitHub account), so the old
/// method stops working. An `External` identity is tied afterwards with `identities::insert`.
#[derive(Debug, Clone, Copy)]
pub enum LoginMethod<'a> {
    Password { hash: &'a str },
    Wallet { address: &'a str },
    Code,
    External,
}

impl LoginMethod<'_> {
    pub fn login_type(&self) -> LoginType {
        match self {
            LoginMethod::Password { .. } => LoginType::Password,
            LoginMethod::Wallet { .. } => LoginType::Wallet,
            LoginMethod::Code => LoginType::Code,
            LoginMethod::External => LoginType::External,
        }
    }
}

pub async fn set_login_method<'e>(
    ex: impl PgExecutor<'e>,
    id: Uuid,
    method: LoginMethod<'_>,
) -> Result<Option<User>, sqlx::Error> {
    let (hash, wallet) = match method {
        LoginMethod::Password { hash } => (Some(hash), None),
        LoginMethod::Wallet { address } => (None, Some(address)),
        LoginMethod::Code | LoginMethod::External => (None, None),
    };
    sqlx::query_as(concat!(
        "WITH gone AS (DELETE FROM user_identities WHERE user_id = $1)
         UPDATE users SET login_type = $2, password_hash = $3, wallet = $4
         WHERE id = $1
         RETURNING ",
        user_columns!()
    ))
    .bind(id)
    .bind(method.login_type())
    .bind(hash)
    .bind(wallet)
    .fetch_optional(ex)
    .await
}

pub async fn touch_last_login<'e>(ex: impl PgExecutor<'e>, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_login_at = now() WHERE id = $1")
        .bind(id)
        .execute(ex)
        .await?;
    Ok(())
}

/// Locks every active admin row and returns their ids. Call inside a transaction before a
/// change that could remove the last active admin, so two such changes cannot both pass.
pub async fn lock_active_admins(conn: &mut PgConnection) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM users WHERE role = 'admin' AND status = 'active' ORDER BY id FOR UPDATE",
    )
    .fetch_all(conn)
    .await
}

/// How `GET /api/users` orders its page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UserSort {
    #[default]
    CreatedDesc,
    CreatedAsc,
    NameAsc,
    NameDesc,
    EmailAsc,
    LastLoginDesc,
}

impl UserSort {
    fn order_by(self) -> &'static str {
        match self {
            UserSort::CreatedDesc => "created_at DESC, id",
            UserSort::CreatedAsc => "created_at ASC, id",
            UserSort::NameAsc => "lower(first_name) ASC, lower(last_name) ASC, id",
            UserSort::NameDesc => "lower(first_name) DESC, lower(last_name) DESC, id",
            UserSort::EmailAsc => "email ASC, id",
            UserSort::LastLoginDesc => "last_login_at DESC NULLS LAST, id",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ListUsers {
    /// Matches anywhere in the email or the full name ("first last"), case-insensitive.
    pub search: Option<String>,
    pub role: Option<Role>,
    pub status: Option<UserStatus>,
    pub sort: UserSort,
    /// Starts at 1.
    pub page: u32,
    pub per_page: u32,
}

/// The most users a list counts. Past it the total reads "10,000+" and paging stops, so no
/// page costs more than this many rows however large the table grows; search and filters
/// reach the rest.
pub const COUNT_CAP: i64 = 10_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    /// Exact up to [`COUNT_CAP`]; `COUNT_CAP` itself when `capped`.
    pub total: i64,
    /// More than [`COUNT_CAP`] users match.
    pub capped: bool,
}

fn push_filters(b: &mut QueryBuilder<sqlx::Postgres>, q: &ListUsers) {
    if let Some(search) = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let pattern = format!("%{}%", app_db::escape_like(search));
        b.push(" AND (email ILIKE ")
            .push_bind(pattern.clone())
            .push(" OR first_name || ' ' || last_name ILIKE ")
            .push_bind(pattern)
            .push(")");
    }
    if let Some(role) = q.role {
        b.push(" AND role = ").push_bind(role);
    }
    if let Some(status) = q.status {
        b.push(" AND status = ").push_bind(status);
    }
}

/// One page of users and how many match, counted up to [`COUNT_CAP`]. A page past the cap is
/// empty. Takes the connection because the count is a second query.
pub async fn list(conn: &mut PgConnection, q: &ListUsers) -> Result<Page<User>, sqlx::Error> {
    let per_page = i64::from(q.per_page.clamp(1, 100));
    let offset = i64::from(q.page.max(1) - 1) * per_page;
    // Never read past the cap, so the cost of a page stays bounded.
    let limit = per_page.min(COUNT_CAP - offset).max(0);
    let mut items: Vec<User> = Vec::new();
    if limit > 0 {
        let mut b = QueryBuilder::new(concat!(
            "SELECT ",
            user_columns!(),
            " FROM users WHERE TRUE"
        ));
        push_filters(&mut b, q);
        b.push(" ORDER BY ")
            .push(q.sort.order_by())
            .push(" LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);
        items = b.build_query_as().fetch_all(&mut *conn).await?;
    }

    // A page shorter than asked, that is not empty or is the first, already says how many match.
    let got = i64::try_from(items.len()).unwrap_or(limit);
    let counted = if got < limit && (offset == 0 || got > 0) {
        offset + got
    } else {
        let mut c = QueryBuilder::new("SELECT count(*) FROM (SELECT 1 FROM users WHERE TRUE");
        push_filters(&mut c, q);
        c.push(" LIMIT ")
            .push_bind(COUNT_CAP + 1)
            .push(") AS capped");
        c.build_query_scalar().fetch_one(&mut *conn).await?
    };
    Ok(Page {
        items,
        total: counted.min(COUNT_CAP),
        capped: counted > COUNT_CAP,
    })
}
