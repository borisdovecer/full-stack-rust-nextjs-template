//! Shared building blocks: configuration, the error type, extractors and the request id.

pub mod config;
pub mod error;
pub mod extract;
pub mod request_id;

pub use config::{AppEnv, Config, Secret, Settings};
pub use error::{AppError, AppResult, ErrorBody, ErrorCode};
