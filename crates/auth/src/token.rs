//! Session tokens: 32 random bytes, base64url in the cookie, SHA-256 in the database.
//! A stolen database therefore holds nothing that signs anyone in.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// A new token and the hash to store for it.
pub fn generate() -> (String, Vec<u8>) {
    let bytes: [u8; 32] = rand::random();
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = hash(&token);
    (token, hash)
}

pub fn hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Cheap shape check before touching the database.
pub fn looks_valid(token: &str) -> bool {
    token.len() == 43
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_well_formed() {
        let (a, ha) = generate();
        let (b, _) = generate();
        assert_ne!(a, b);
        assert!(looks_valid(&a));
        assert_eq!(ha, hash(&a));
        assert_eq!(ha.len(), 32);
    }

    #[test]
    fn shape_check() {
        assert!(!looks_valid(""));
        assert!(!looks_valid(&"a".repeat(44)));
        assert!(!looks_valid(&format!("{}=", "a".repeat(42))));
    }
}
