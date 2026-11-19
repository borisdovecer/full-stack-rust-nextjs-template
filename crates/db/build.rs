//! `sqlx::migrate!` reads `migrations/` at compile time, so cargo must rebuild this crate when
//! a file there is added, removed or edited. Without this, a new migration leaves the binary
//! and the tests without it, and `migrate` still reports the database as current.

fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
