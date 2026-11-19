//! Password hashing with Argon2id at the library's defaults (OWASP's recommended parameters).
//! Hashing is CPU-heavy: call these from `tokio::task::spawn_blocking` in async code.

use std::sync::LazyLock;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};

pub const MIN_LENGTH: usize = 12;
pub const MAX_LENGTH: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordRuleError {
    #[error("the password must be at least {MIN_LENGTH} characters")]
    TooShort,
    #[error("the password must be at most {MAX_LENGTH} characters")]
    TooLong,
}

impl PasswordRuleError {
    /// The problem code a client sees in `details.fields`.
    pub fn code(self) -> &'static str {
        match self {
            PasswordRuleError::TooShort => "too_short",
            PasswordRuleError::TooLong => "too_long",
        }
    }
}

/// Length is the only rule: 12 to 128 characters, counted as characters, not bytes.
pub fn check_rules(password: &str) -> Result<(), PasswordRuleError> {
    let n = password.chars().count();
    if n < MIN_LENGTH {
        Err(PasswordRuleError::TooShort)
    } else if n > MAX_LENGTH {
        Err(PasswordRuleError::TooLong)
    } else {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("password hashing failed")]
pub struct HashError;

/// A PHC string (`$argon2id$v=19$...`) with a fresh random salt.
pub fn hash(password: &str) -> Result<String, HashError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|_| HashError)
}

/// False for a wrong password and for anything that is not a valid hash.
pub fn verify(phc: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

static DUMMY: LazyLock<String> =
    LazyLock::new(|| hash("not-a-real-password-just-timing").unwrap_or_default());

/// Spends the same time as checking a real password. Used when the account does not exist,
/// so a response time cannot tell whether an email has an account.
pub fn verify_dummy(password: &str) {
    let _ = verify(&DUMMY, password);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let h = hash("correct horse battery").unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(verify(&h, "correct horse battery"));
        assert!(!verify(&h, "wrong horse battery"));
    }

    #[test]
    fn same_password_gets_different_hashes() {
        assert_ne!(
            hash("same password!!").unwrap(),
            hash("same password!!").unwrap()
        );
    }

    #[test]
    fn garbage_hash_never_verifies() {
        assert!(!verify("not a hash", "anything"));
        assert!(!verify("", ""));
    }

    #[test]
    fn length_rules_count_characters() {
        assert_eq!(check_rules("short"), Err(PasswordRuleError::TooShort));
        assert_eq!(check_rules(&"a".repeat(12)), Ok(()));
        assert_eq!(check_rules(&"ž".repeat(12)), Ok(()));
        assert_eq!(
            check_rules(&"a".repeat(129)),
            Err(PasswordRuleError::TooLong)
        );
    }
}
