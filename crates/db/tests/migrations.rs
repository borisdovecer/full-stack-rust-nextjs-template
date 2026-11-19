//! The migrations against a real database. Each test gets a fresh database.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use sqlx::PgPool;

use app_db::{MIGRATOR, MigrationState};

#[sqlx::test(migrations = false)]
async fn migrations_go_up_down_and_up_again(pool: PgPool) {
    assert!(matches!(
        app_db::migration_state(&pool).await.unwrap(),
        MigrationState::Pending(_)
    ));
    MIGRATOR.run(&pool).await.unwrap();
    assert_eq!(
        app_db::migration_state(&pool).await.unwrap(),
        MigrationState::Current
    );

    MIGRATOR.undo(&pool, 0).await.unwrap();
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.tables
         WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(tables, 0, "a down migration left a table behind");

    MIGRATOR.run(&pool).await.unwrap();
    assert_eq!(
        app_db::migration_state(&pool).await.unwrap(),
        MigrationState::Current
    );
}

#[sqlx::test(migrator = "app_db::MIGRATOR")]
async fn an_edited_migration_is_detected(pool: PgPool) {
    sqlx::query("UPDATE _sqlx_migrations SET checksum = '\\x00' WHERE version = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        app_db::migration_state(&pool).await.unwrap(),
        MigrationState::Changed(1)
    );
}
