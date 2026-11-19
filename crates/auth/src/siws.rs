//! Sign-In-With-Solana: the message a wallet signs, and the checks on it.
//!
//! The text follows the wallet-standard format (`createSignInMessageText` in
//! `@solana/wallet-standard-util`), so a wallet's own `signIn` and a `signMessage` fallback built
//! in the browser produce the same bytes. The server never trusts the text: it parses it and
//! checks the domain, address, nonce and times against what it issued, then the signature over
//! the exact bytes.

use chrono::{DateTime, Duration, Utc};
use ed25519_dalek::{Signature, VerifyingKey};

const HEADER_END: &str = " wants you to sign in with your Solana account:";
/// How far the message's Issued At may be from the server's clock.
const ISSUED_AT_TOLERANCE_MINUTES: i64 = 10;
const FIELD_KEYS: [&str; 9] = [
    "URI",
    "Version",
    "Chain ID",
    "Nonce",
    "Issued At",
    "Expiration Time",
    "Not Before",
    "Request ID",
    "Resources",
];

/// The parts of a sign-in message the server checks.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SiwsMessage {
    pub domain: String,
    pub address: String,
    pub statement: Option<String>,
    pub nonce: Option<String>,
    pub issued_at: Option<String>,
    pub expiration_time: Option<String>,
}

/// What a sign-in message must match.
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    pub domain: &'a str,
    pub address: &'a str,
    pub nonce: &'a str,
    pub now: DateTime<Utc>,
}

/// Why a signed message was refused. For logs; a person sees one generic answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SiwsError {
    #[error("not a sign-in message")]
    Unparseable,
    #[error("signed for another domain")]
    Domain,
    #[error("signed for another address, or the address is not a Solana key")]
    Address,
    #[error("the nonce is not the one issued")]
    Nonce,
    #[error("issued at a time too far from now")]
    IssuedAt,
    #[error("expired")]
    Expired,
    #[error("the signature does not match")]
    Signature,
}

/// The 32-byte Ed25519 key of a base58 Solana address.
pub fn decode_address(address: &str) -> Option<[u8; 32]> {
    let bytes = bs58::decode(address).into_vec().ok()?;
    bytes.try_into().ok()
}

/// A fresh nonce: 16 random bytes in base58, alphanumeric as the format requires.
pub fn new_nonce() -> String {
    let bytes: [u8; 16] = rand::random();
    bs58::encode(bytes).into_string()
}

pub fn parse(text: &str) -> Option<SiwsMessage> {
    let (head, rest) = text.split_once('\n')?;
    let domain = head.strip_suffix(HEADER_END)?.trim();
    let (address, rest) = rest.split_once('\n').unwrap_or((rest, ""));
    let address = address.trim();
    if domain.is_empty() || address.is_empty() {
        return None;
    }
    let mut msg = SiwsMessage {
        domain: domain.to_string(),
        address: address.to_string(),
        ..Default::default()
    };
    for block in rest.split("\n\n").map(str::trim).filter(|b| !b.is_empty()) {
        let is_fields = FIELD_KEYS
            .iter()
            .any(|k| block.starts_with(&format!("{k}:")));
        if !is_fields {
            if msg.statement.is_none() {
                msg.statement = Some(block.to_string());
            }
            continue;
        }
        for line in block.lines() {
            let Some((key, value)) = line.split_once(": ") else {
                continue;
            };
            let value = Some(value.trim().to_string());
            match key {
                "Nonce" => msg.nonce = value,
                "Issued At" => msg.issued_at = value,
                "Expiration Time" => msg.expiration_time = value,
                _ => {}
            }
        }
    }
    Some(msg)
}

/// Checks a signed message against what the server issued. The nonce must already have been
/// spent by the caller, so a refused message cannot be tried again.
pub fn verify(
    message: &[u8],
    signature: &[u8],
    expected: Expected<'_>,
) -> Result<SiwsMessage, SiwsError> {
    let text = std::str::from_utf8(message).map_err(|_| SiwsError::Unparseable)?;
    let msg = parse(text).ok_or(SiwsError::Unparseable)?;
    if msg.domain != expected.domain {
        return Err(SiwsError::Domain);
    }
    if msg.address != expected.address {
        return Err(SiwsError::Address);
    }
    if msg.nonce.as_deref() != Some(expected.nonce) {
        return Err(SiwsError::Nonce);
    }
    let issued_at = msg
        .issued_at
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .ok_or(SiwsError::IssuedAt)?;
    let tolerance = Duration::minutes(ISSUED_AT_TOLERANCE_MINUTES);
    if (expected.now - issued_at.with_timezone(&Utc)).abs() > tolerance {
        return Err(SiwsError::IssuedAt);
    }
    if let Some(exp) = msg.expiration_time.as_deref() {
        let exp = DateTime::parse_from_rfc3339(exp).map_err(|_| SiwsError::Expired)?;
        if exp.with_timezone(&Utc) <= expected.now {
            return Err(SiwsError::Expired);
        }
    }
    let key = decode_address(&msg.address)
        .and_then(|k| VerifyingKey::from_bytes(&k).ok())
        .ok_or(SiwsError::Address)?;
    let signature: [u8; 64] = signature.try_into().map_err(|_| SiwsError::Signature)?;
    key.verify_strict(message, &Signature::from_bytes(&signature))
        .map_err(|_| SiwsError::Signature)?;
    Ok(msg)
}

/// The message text for these fields, exactly as `createSignInMessageText` writes it.
pub fn message_text(msg: &SiwsMessage) -> String {
    let mut text = format!("{}{HEADER_END}\n{}", msg.domain, msg.address);
    if let Some(statement) = &msg.statement {
        text.push_str(&format!("\n\n{statement}"));
    }
    let mut fields = Vec::new();
    if let Some(n) = &msg.nonce {
        fields.push(format!("Nonce: {n}"));
    }
    if let Some(t) = &msg.issued_at {
        fields.push(format!("Issued At: {t}"));
    }
    if let Some(t) = &msg.expiration_time {
        fields.push(format!("Expiration Time: {t}"));
    }
    if !fields.is_empty() {
        text.push_str(&format!("\n\n{}", fields.join("\n")));
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn address(k: &SigningKey) -> String {
        bs58::encode(k.verifying_key().as_bytes()).into_string()
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2030-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn message(k: &SigningKey) -> SiwsMessage {
        SiwsMessage {
            domain: "app.example.com".into(),
            address: address(k),
            statement: Some("Sign in to App. This is a signature, not a transaction.".into()),
            nonce: Some("Nonce123abc".into()),
            issued_at: Some("2030-01-01T11:59:00.000Z".into()),
            expiration_time: Some("2030-01-01T12:04:00.000Z".into()),
        }
    }

    fn expected(addr: &str) -> Expected<'_> {
        Expected {
            domain: "app.example.com",
            address: addr,
            nonce: "Nonce123abc",
            now: now(),
        }
    }

    fn signed(k: &SigningKey, text: &str) -> Vec<u8> {
        k.sign(text.as_bytes()).to_bytes().to_vec()
    }

    #[test]
    fn a_good_message_passes() {
        let k = key();
        let addr = address(&k);
        let text = message_text(&message(&k));
        let parsed = verify(text.as_bytes(), &signed(&k, &text), expected(&addr)).unwrap();
        assert_eq!(parsed, message(&k));
    }

    #[test]
    fn parses_what_wallets_add() {
        let k = key();
        let text = format!(
            "app.example.com wants you to sign in with your Solana account:\n{}\n\nA statement: with a colon\n\nURI: https://app.example.com\nVersion: 1\nChain ID: mainnet\nNonce: abc12345\nIssued At: 2030-01-01T12:00:00Z\nResources:\n- https://x",
            address(&k)
        );
        let m = parse(&text).unwrap();
        assert_eq!(m.statement.as_deref(), Some("A statement: with a colon"));
        assert_eq!(m.nonce.as_deref(), Some("abc12345"));
        assert_eq!(m.issued_at.as_deref(), Some("2030-01-01T12:00:00Z"));
        assert_eq!(parse("hello"), None);
        assert_eq!(
            parse("x wants you to sign in with your Ethereum account:\n0xabc"),
            None
        );
    }

    #[test]
    fn each_mismatch_is_refused() {
        let k = key();
        let addr = address(&k);
        let check = |m: SiwsMessage| {
            let text = message_text(&m);
            verify(text.as_bytes(), &signed(&k, &text), expected(&addr)).unwrap_err()
        };
        let base = message(&k);
        assert_eq!(
            check(SiwsMessage {
                domain: "evil.example".into(),
                ..base.clone()
            }),
            SiwsError::Domain
        );
        assert_eq!(
            check(SiwsMessage {
                nonce: Some("other".into()),
                ..base.clone()
            }),
            SiwsError::Nonce
        );
        assert_eq!(
            check(SiwsMessage {
                issued_at: Some("2030-01-01T11:40:00Z".into()),
                ..base.clone()
            }),
            SiwsError::IssuedAt
        );
        assert_eq!(
            check(SiwsMessage {
                issued_at: None,
                ..base.clone()
            }),
            SiwsError::IssuedAt
        );
        assert_eq!(
            check(SiwsMessage {
                expiration_time: Some("2030-01-01T11:59:59Z".into()),
                ..base.clone()
            }),
            SiwsError::Expired
        );
        let other = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(
            check(SiwsMessage {
                address: address(&other),
                ..base
            }),
            SiwsError::Address
        );
    }

    #[test]
    fn the_signature_covers_the_exact_bytes() {
        let k = key();
        let addr = address(&k);
        let text = message_text(&message(&k));
        let sig = signed(&k, &text);
        let tampered = text.replace("Sign in", "Sign up");
        assert_eq!(
            verify(tampered.as_bytes(), &sig, expected(&addr)).unwrap_err(),
            SiwsError::Signature
        );
        let other = SigningKey::from_bytes(&[9u8; 32]);
        assert_eq!(
            verify(text.as_bytes(), &signed(&other, &text), expected(&addr)).unwrap_err(),
            SiwsError::Signature
        );
        assert_eq!(
            verify(text.as_bytes(), &sig[..63], expected(&addr)).unwrap_err(),
            SiwsError::Signature
        );
        assert_eq!(
            verify(&[0xff, 0xfe], &sig, expected(&addr)).unwrap_err(),
            SiwsError::Unparseable
        );
    }

    #[test]
    fn addresses_and_nonces() {
        assert!(decode_address(&address(&key())).is_some());
        assert!(decode_address("not-base58-0OIl").is_none());
        assert!(decode_address("abc").is_none());
        let n = new_nonce();
        assert!(n.len() >= 16 && n.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(n, new_nonce());
    }
}
