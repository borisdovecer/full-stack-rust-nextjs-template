//! The API service as a library, so integration tests can build the router.
//!
//! Code is grouped by scope: one folder per URL prefix (`auth`, `me`, `users`, `help`,
//! `system`). `docs/rules/architecture.md` says what goes where.

pub mod app;
pub mod auth;
pub mod cli;
pub mod help;
pub mod jobs;
pub mod me;
pub mod server;
pub mod shared;
pub mod state;
pub mod system;
pub mod telemetry;
pub mod users;
