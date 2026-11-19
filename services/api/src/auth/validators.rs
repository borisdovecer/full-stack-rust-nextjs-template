//! Checks for sign-in proofs and new passwords. They extend [`FieldErrors`], so a route
//! collects them with the shared checks and answers every bad field at once.

use app_auth::password;

use super::repository::LoginType;
use super::service::wallet;
use crate::shared::validation::FieldErrors;

/// The proof fields a request may carry, whatever the method.
#[derive(Debug, Default)]
pub struct ProofFields {
    pub password: Option<String>,
    pub login_code: Option<String>,
    pub address: Option<String>,
    pub message: Option<String>,
    pub signature: Option<String>,
}

/// The proof a method needs, once its fields passed [`FieldErrors::method_proof`].
#[derive(Debug, Default)]
pub struct MethodProof {
    pub password: Option<String>,
    pub login_code: Option<String>,
    /// Address (normalised), message and signature.
    pub wallet: Option<(String, String, String)>,
}

impl FieldErrors {
    /// A new password that must be present; records `required` when it is missing.
    pub fn required_new_password(
        &mut self,
        field: &'static str,
        pw: Option<String>,
    ) -> Option<String> {
        match pw {
            Some(pw) => {
                self.new_password(field, &pw);
                Some(pw)
            }
            None => {
                self.add(field, "required");
                None
            }
        }
    }

    /// The three parts of a signed wallet challenge: a Solana address (normalised), and the
    /// message and signature, which are checked later against the challenge.
    pub fn wallet_proof(
        &mut self,
        address: Option<&str>,
        message: Option<&str>,
        signature: Option<&str>,
    ) -> Option<(String, String, String)> {
        let address = match address {
            None => {
                self.add("address", "required");
                None
            }
            Some(raw) => {
                let a = wallet::normalize_address(raw);
                if a.is_none() {
                    self.add("address", "invalid_address");
                }
                a
            }
        };
        if message.is_none() {
            self.add("message", "required");
        }
        if signature.is_none() {
            self.add("signature", "required");
        }
        Some((address?, message?.to_string(), signature?.to_string()))
    }

    /// The fields the proof of `method` needs, for registering or switching to it: the new
    /// password for `password`, the code's presence for `code`, the signed challenge for
    /// `wallet`. `external` proves itself elsewhere and needs none.
    pub fn method_proof(&mut self, method: LoginType, given: ProofFields) -> MethodProof {
        let mut out = MethodProof::default();
        match method {
            LoginType::Password => {
                out.password = self.required_new_password("password", given.password);
            }
            LoginType::Code => match given.login_code {
                Some(code) => out.login_code = Some(code),
                None => self.add("login_code", "required"),
            },
            LoginType::Wallet => {
                out.wallet = self.wallet_proof(
                    given.address.as_deref(),
                    given.message.as_deref(),
                    given.signature.as_deref(),
                );
            }
            LoginType::External => {}
        }
        out
    }

    pub fn new_password(&mut self, field: &'static str, pw: &str) {
        if let Err(e) = password::check_rules(pw) {
            self.add(field, e.code());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_password_is_too_short() {
        let mut f = FieldErrors::default();
        f.new_password("password", "short");
        let err = f.check().unwrap_err();
        assert_eq!(err.details().unwrap()["fields"]["password"][0], "too_short");
    }
}
