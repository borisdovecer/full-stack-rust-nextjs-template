//! Which sign-in methods this server offers. A method is listed once its routes exist and its
//! settings are complete; the web draws only listed methods and the API refuses the others.

use app_core::{AppResult, ErrorCode};

use crate::auth::repository::LoginType;
use crate::state::AppState;

/// The methods a person can register and sign in with, in the order the web shows them.
pub fn available(state: &AppState) -> Vec<LoginType> {
    let mut methods = vec![LoginType::Password];
    // Codes written to the log would sit in a server's log files, so production needs real mail.
    if state.mailer.delivers() || !state.config.env.is_production() {
        methods.push(LoginType::Code);
    }
    methods.push(LoginType::Wallet);
    if !crate::auth::oauth::enabled(&state.config).is_empty() {
        methods.push(LoginType::External);
    }
    methods
}

/// Refuses a method that is not available.
pub fn require(state: &AppState, method: LoginType) -> AppResult<()> {
    if available(state).contains(&method) {
        Ok(())
    } else {
        Err(ErrorCode::LoginMethodUnavailable.into())
    }
}

/// Methods an admin may give an account without the person present: both need no secret from
/// the person beyond what the admin sets or what reaches their inbox.
pub fn admin_assignable(state: &AppState) -> Vec<LoginType> {
    available(state)
        .into_iter()
        .filter(|m| matches!(m, LoginType::Password | LoginType::Code))
        .collect()
}

/// Refuses a method an admin may not give.
pub fn require_admin_assignable(state: &AppState, method: LoginType) -> AppResult<()> {
    if admin_assignable(state).contains(&method) {
        Ok(())
    } else {
        Err(ErrorCode::LoginMethodUnavailable.into())
    }
}
