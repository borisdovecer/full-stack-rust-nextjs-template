# Database rules

How the schema changes. What exists today: `docs/system/database.md`.

## Migrations

- Every table, column, index, type and constraint is created by a file in `migrations/`. Nothing is changed by hand on a shared database.
- Name: `NNNN_short_description.up.sql` and `NNNN_short_description.down.sql`, the next free number, snake case.
- The down file removes exactly what its up file added. The test `migrations_go_up_down_and_up_again` runs every migration up, down and up again.
- A migration that has run on any database is never edited. A fix is a new migration. The API detects an edited file by its checksum and refuses to start.
- Each up file starts with a comment: what it does and why, the bug report or feature request it belongs to, and, for a new table, the service that owns it (`architecture.md`).
- Apply with `app-api migrate`. Undo the newest with `app-api migrate --revert-last` (development only; in production, write a new migration).

## Changing tables safely

The API and the database are deployed separately, so a migration must work with the code before and after it.

- **New column**: nullable or with a default. Never `NOT NULL` without a default on a table with rows.
- **Renaming or dropping a column**: in steps. Add the new one, write both, move readers, then drop the old one in a later migration.
- **New index on a big table**: `CREATE INDEX CONCURRENTLY` in a migration of its own (it cannot run in a transaction; mark the file with `-- no-transaction` as the first line).
- **Data changes** go in their own migration, separate from schema changes, and must be safe to run once.
- **Enums**: adding a value is cheap; removing or renaming one needs the step-by-step approach above.

## Naming

- Tables plural snake case (`user_sessions`), columns snake case, foreign keys `<thing>_id`.
- Times are `timestamptz` and end in `_at`. Booleans read as a fact (`success`, not `is_success_flag`).
- Constraints and indexes: `<table>_<columns>_<kind>`: `_key` unique, `_idx` index, `_fkey` foreign key, a word for checks (`users_email_normalised`).

## Queries

- A table's SQL lives in the `repository.rs` of the scope that owns it, and only its owner service queries it (`architecture.md`). Handlers never write SQL.
- Bind every value. Never build SQL from input; dynamic filters use `QueryBuilder` with `push_bind`.
- Several statements that must succeed together run in one transaction, and the audit event for a change is written in the same transaction.
- Every query function has a test in `services/<service>/tests/repository.rs` or through the route that uses it. Migration tests stay in `crates/db/tests/`.
