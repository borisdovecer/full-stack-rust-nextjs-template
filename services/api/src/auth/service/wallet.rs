//! Solana wallet sign-in: issuing a challenge and checking a signed one.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::json;
use utoipa::ToSchema;

use app_auth::siws::{self, Expected};
use app_core::{AppError, AppResult, ErrorCode};

use crate::auth::repository::wallet_nonces;
use crate::state::AppState;

/// What the wallet signs. The web passes these to the wallet's `signIn`, or builds the same
/// text with `createSignInMessageText` for `signMessage`.
#[derive(Debug, Serialize, ToSchema)]
pub struct WalletChallenge {
    pub domain: String,
    pub address: String,
    pub statement: String,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expiration_time: DateTime<Utc>,
}

/// A signed challenge as the browser sends it.
#[derive(Debug, Clone, Copy)]
pub struct Proof<'a> {
    pub address: &'a str,
    /// The exact bytes the wallet signed, base64.
    pub message: &'a str,
    /// The 64-byte Ed25519 signature, base64.
    pub signature: &'a str,
}

/// Issues a challenge for `address`. Refuses when the IP asked for too many.
pub async fn challenge(
    state: &AppState,
    address: &str,
    ip: Option<&str>,
) -> AppResult<WalletChallenge> {
    let w = &state.config.settings.wallet;
    let now = Utc::now();
    let lifetime = Duration::minutes(i64::from(w.nonce_minutes));
    if let Some(ip) = ip {
        let (count, oldest) = wallet_nonces::issued_to_ip(&state.db, ip, now - lifetime).await?;
        if count >= i64::from(w.max_nonces_per_ip) {
            let retry_at = oldest.unwrap_or(now) + lifetime;
            return Err(AppError::new(ErrorCode::TooManyAttempts)
                .with_details(json!({ "retry_at": retry_at })));
        }
    }
    let nonce = siws::new_nonce();
    let expires_at = now + lifetime;
    let issued_at = wallet_nonces::create(&state.db, &nonce, address, ip, expires_at).await?;
    Ok(WalletChallenge {
        domain: w.domain.clone(),
        address: address.to_string(),
        statement: w.statement.clone(),
        nonce,
        issued_at,
        expiration_time: expires_at,
    })
}

/// True when `proof` is a signature, by `proof.address`, over a challenge this server issued
/// for that address. The challenge is spent first, so a refused proof cannot be retried.
pub async fn verify(state: &AppState, proof: Proof<'_>) -> AppResult<bool> {
    let (Ok(message), Ok(signature)) = (
        STANDARD.decode(proof.message),
        STANDARD.decode(proof.signature),
    ) else {
        tracing::info!(address = proof.address, "wallet proof refused: not base64");
        return Ok(false);
    };
    let Some(nonce) = std::str::from_utf8(&message)
        .ok()
        .and_then(siws::parse)
        .and_then(|m| m.nonce)
    else {
        tracing::info!(
            address = proof.address,
            "wallet proof refused: not a sign-in message"
        );
        return Ok(false);
    };
    if !wallet_nonces::consume(&state.db, &nonce, proof.address).await? {
        tracing::info!(
            address = proof.address,
            "wallet proof refused: unknown, used or expired nonce"
        );
        return Ok(false);
    }
    let expected = Expected {
        domain: &state.config.settings.wallet.domain,
        address: proof.address,
        nonce: &nonce,
        now: Utc::now(),
    };
    match siws::verify(&message, &signature, expected) {
        Ok(_) => Ok(true),
        Err(reason) => {
            tracing::info!(address = proof.address, %reason, "wallet proof refused");
            Ok(false)
        }
    }
}

/// A trimmed Solana address, or `None` when it is not a 32-byte base58 key.
pub fn normalize_address(raw: &str) -> Option<String> {
    let address = raw.trim();
    siws::decode_address(address).map(|_| address.to_string())
}
