# migrations/

Every table, column, index and type in the database is created by a numbered SQL file here. The tables are described in `docs/system/database.md`.

## Rules

- Name: `NNNN_description.up.sql` with a matching `NNNN_description.down.sql`. Four digits, then snake case.
- The down file removes what its up file added, and nothing else. A test runs every migration up, down and up again.
- A file that has run on any database is never edited. A change is a new file with the next number. The API detects an edited file (checksum) and refuses to start.
- Each up file starts with a comment: what it does and why, and the bug report or feature request it belongs to.
- A new column on an existing table has a default or is nullable, so the old code keeps working while the deploy rolls out.

## Running them

The files are built into the API binary, so the same commands work on a laptop and a server:

```bash
cargo run -p app-api -- migrate                 # apply pending migrations
cargo run -p app-api -- migrate --revert-last   # undo the newest one
```

The files are read when `crates/db` compiles; `crates/db/build.rs` tells cargo to rebuild it whenever this folder changes, so a new file is never left out of the binary or the tests.

`scripts/setup.sh` runs `migrate` for you. The API checks at start and refuses to run with a pending, failed or edited migration; `GET /api/health` reports the same.
