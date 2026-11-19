//! Database plumbing every service shares: the pool, the embedded migrations, and query
//! helpers. Tables and their SQL live in the service that owns them.

use std::time::Duration;

use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

pub use sqlx::{PgConnection, PgPool as Pool, Postgres, Transaction};

/// Every migration in `migrations/`, built into the binary.
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

/// How the pool is sized and how long a statement may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolSettings {
    pub max_connections: u32,
    /// How long a request waits for a free connection before `503 unavailable`.
    pub acquire_timeout: Duration,
    /// Postgres ends a statement that runs longer. `None` for no limit (migrations).
    pub statement_timeout: Option<Duration>,
}

impl Default for PoolSettings {
    fn default() -> Self {
        Self {
            max_connections: 10,
            acquire_timeout: Duration::from_secs(5),
            statement_timeout: Some(Duration::from_secs(30)),
        }
    }
}

pub async fn connect(url: &str, settings: PoolSettings) -> Result<PgPool, sqlx::Error> {
    let mut options: PgConnectOptions = url.parse()?;
    if let Some(timeout) = settings.statement_timeout {
        options = options.options([("statement_timeout", format!("{}ms", timeout.as_millis()))]);
    }
    PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(settings.acquire_timeout)
        .connect_with(options)
        .await
}

/// The applied migration versions, newest first.
pub async fn applied_migrations(pool: &PgPool) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version DESC")
        .fetch_all(pool)
        .await
}

/// Takes a session-level advisory lock if nobody holds it. Held until [`advisory_unlock`] or
/// until the connection closes, so a crashed holder never keeps it.
pub async fn try_advisory_lock(conn: &mut PgConnection, key: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(key)
        .fetch_one(conn)
        .await
}

pub async fn advisory_unlock(conn: &mut PgConnection, key: i64) -> Result<(), sqlx::Error> {
    sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(conn)
        .await?;
    Ok(())
}

/// True when `err` is a unique violation of the named constraint.
pub fn is_unique_violation(err: &sqlx::Error, constraint: &str) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.is_unique_violation() && db.constraint() == Some(constraint)
        }
        _ => false,
    }
}

/// How the database compares with the migrations built into this binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    Current,
    /// Versions not applied yet.
    Pending(Vec<i64>),
    /// A migration started and failed; the database needs a person.
    Failed(i64),
    /// An applied migration's file was edited afterwards.
    Changed(i64),
    /// The database has a migration this binary does not know (an older binary on a newer
    /// database).
    Unknown(i64),
}

impl MigrationState {
    pub fn is_current(&self) -> bool {
        *self == MigrationState::Current
    }

    pub fn describe(&self) -> String {
        match self {
            MigrationState::Current => "migrations are current".into(),
            MigrationState::Pending(v) => {
                format!("pending migrations {v:?}: run `app-api migrate`")
            }
            MigrationState::Failed(v) => format!("migration {v} failed part-way: fix it by hand"),
            MigrationState::Changed(v) => {
                format!("migration {v} was edited after it ran: restore the file, add a new one")
            }
            MigrationState::Unknown(v) => {
                format!("the database has migration {v}, which this build does not know")
            }
        }
    }
}

pub async fn migration_state(pool: &PgPool) -> Result<MigrationState, sqlx::Error> {
    let known: Vec<_> = MIGRATOR
        .iter()
        .filter(|m| m.migration_type.is_up_migration())
        .collect();

    let has_table: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(pool)
        .await?;
    if !has_table {
        return Ok(MigrationState::Pending(
            known.iter().map(|m| m.version).collect(),
        ));
    }

    let applied: Vec<(i64, Vec<u8>, bool)> =
        sqlx::query_as("SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;

    for (version, checksum, success) in &applied {
        if !success {
            return Ok(MigrationState::Failed(*version));
        }
        match known.iter().find(|m| m.version == *version) {
            None => return Ok(MigrationState::Unknown(*version)),
            Some(m) if m.checksum.as_ref() != checksum.as_slice() => {
                return Ok(MigrationState::Changed(*version));
            }
            Some(_) => {}
        }
    }

    let pending: Vec<i64> = known
        .iter()
        .map(|m| m.version)
        .filter(|v| !applied.iter().any(|(a, _, _)| a == v))
        .collect();
    Ok(if pending.is_empty() {
        MigrationState::Current
    } else {
        MigrationState::Pending(pending)
    })
}

/// `%`, `_` and `\` taken literally inside a LIKE pattern.
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_escaping() {
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    #[test]
    fn every_up_migration_has_a_down() {
        let ups: Vec<i64> = MIGRATOR
            .iter()
            .filter(|m| m.migration_type.is_up_migration())
            .map(|m| m.version)
            .collect();
        let downs: Vec<i64> = MIGRATOR
            .iter()
            .filter(|m| m.migration_type.is_down_migration())
            .map(|m| m.version)
            .collect();
        assert!(!ups.is_empty());
        assert_eq!(ups, downs);
    }
}
