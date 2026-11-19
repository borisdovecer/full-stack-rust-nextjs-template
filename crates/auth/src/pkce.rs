//! OAuth 2 helpers: the PKCE pair (RFC 7636, S256) and the `state` value.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// A random verifier (43 base64url characters) and its S256 challenge.
pub fn pair() -> (String, String) {
    let bytes: [u8; 32] = rand::random();
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let challenge = challenge(&verifier);
    (verifier, challenge)
}

pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// A random `state` that ties a provider's answer to the browser that asked.
pub fn state() -> String {
    let bytes: [u8; 24] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_7636_example() {
        // Appendix B of RFC 7636.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn pairs_and_states_are_fresh() {
        let (v, c) = pair();
        assert_eq!(v.len(), 43);
        assert_eq!(c, challenge(&v));
        assert_ne!(pair().0, v);
        assert_ne!(state(), state());
    }
}
