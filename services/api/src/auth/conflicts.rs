//! Unique violations that are the client's to fix, as their error codes.

use app_core::{AppError, ErrorCode};
use app_db::is_unique_violation;

use super::repository::{identities, users};

/// `email_taken`, `wallet_taken` or `identity_taken` for a violation of their key; any other
/// database error as itself.
pub fn map(e: sqlx::Error) -> AppError {
    let code = if is_unique_violation(&e, users::EMAIL_KEY) {
        ErrorCode::EmailTaken
    } else if is_unique_violation(&e, users::WALLET_KEY) {
        ErrorCode::WalletTaken
    } else if is_unique_violation(&e, identities::IDENTITY_KEY) {
        ErrorCode::IdentityTaken
    } else {
        return AppError::from(e);
    };
    AppError::new(code)
}
