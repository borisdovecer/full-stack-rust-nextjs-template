//! One-time sign-in codes: six digits, short-lived, stored only as a salted SHA-256.
//!
//! A six-digit code is guessable offline from its hash, so the hash only keeps codes out of
//! plain sight in a database copy; what protects a code is its few minutes of life and its
//! few tries. Each row's id is the salt, so two equal codes never share a hash.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const LEN: usize = 6;

/// A new code, every value from `000000` to `999999` equally likely.
pub fn generate() -> String {
    format!("{:06}", rand::random_range(0..1_000_000u32))
}

pub fn hash(salt: &[u8], code: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(salt);
    h.update(code.as_bytes());
    h.finalize().to_vec()
}

pub fn verify(salt: &[u8], code: &str, stored: &[u8]) -> bool {
    hash(salt, code).ct_eq(stored).into()
}

/// Six ASCII digits.
pub fn looks_valid(code: &str) -> bool {
    code.len() == LEN && code.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_six_digits() {
        for _ in 0..200 {
            let c = generate();
            assert!(looks_valid(&c), "{c}");
        }
        assert!(!looks_valid("12345"));
        assert!(!looks_valid("12345a"));
        assert!(!looks_valid("1234567"));
    }

    #[test]
    fn hashes_are_salted_and_verify() {
        let h = hash(b"row-1", "123456");
        assert_ne!(h, hash(b"row-2", "123456"));
        assert!(verify(b"row-1", "123456", &h));
        assert!(!verify(b"row-1", "123457", &h));
        assert!(!verify(b"row-2", "123456", &h));
    }
}
