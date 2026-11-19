//! Google and GitHub: the redirect to the provider, and turning its answer into a verified
//! account (provider user id, email, names). No OAuth library: two providers, two requests each.

use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;

use app_core::config::Secret;
use app_core::config::{Config, OauthEndpoints, OauthProvider};

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Github,
}

/// A provider account, as the provider vouches for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccount {
    /// The provider's own, permanent user id.
    pub subject: String,
    /// Normalised.
    pub email: String,
    pub email_verified: bool,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("the provider request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("the provider answered without {0}")]
    Missing(&'static str),
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Google, Provider::Github];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "google" => Some(Provider::Google),
            "github" => Some(Provider::Github),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Google => "google",
            Provider::Github => "github",
        }
    }

    fn settings(self, config: &Config) -> &OauthProvider {
        match self {
            Provider::Google => &config.settings.oauth.google,
            Provider::Github => &config.settings.oauth.github,
        }
    }

    fn secret(self, config: &Config) -> Option<&Secret> {
        match self {
            Provider::Google => config.google_client_secret.as_ref(),
            Provider::Github => config.github_client_secret.as_ref(),
        }
    }

    /// On when both its client id and its secret are set.
    pub fn enabled(self, config: &Config) -> bool {
        !self.settings(config).client_id.trim().is_empty() && self.secret(config).is_some()
    }

    fn endpoints(self, config: &Config) -> OauthEndpoints {
        if let Some(e) = &self.settings(config).endpoints {
            return e.clone();
        }
        match self {
            Provider::Google => OauthEndpoints {
                authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_url: "https://oauth2.googleapis.com/token".into(),
                user_url: "https://openidconnect.googleapis.com/v1/userinfo".into(),
                emails_url: None,
            },
            Provider::Github => OauthEndpoints {
                authorize_url: "https://github.com/login/oauth/authorize".into(),
                token_url: "https://github.com/login/oauth/access_token".into(),
                user_url: "https://api.github.com/user".into(),
                emails_url: Some("https://api.github.com/user/emails".into()),
            },
        }
    }

    fn scope(self) -> &'static str {
        match self {
            Provider::Google => "openid email profile",
            Provider::Github => "read:user user:email",
        }
    }

    pub fn redirect_uri(self, config: &Config) -> String {
        format!(
            "{}/api/auth/oauth/{}/callback",
            config.settings.oauth.public_url,
            self.as_str()
        )
    }

    /// Where to send the browser to sign in at the provider.
    pub fn authorize_url(self, config: &Config, state: &str, challenge: &str) -> String {
        let e = self.endpoints(config);
        let redirect = self.redirect_uri(config);
        let mut params = vec![
            ("client_id", self.settings(config).client_id.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("response_type", "code"),
            ("scope", self.scope()),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
        ];
        if self == Provider::Google {
            params.push(("prompt", "select_account"));
        }
        match reqwest::Url::parse_with_params(&e.authorize_url, &params) {
            Ok(url) => url.into(),
            Err(_) => e.authorize_url,
        }
    }

    /// Exchanges the code from the callback and reads the account it opens.
    pub async fn account(
        self,
        state: &AppState,
        code: &str,
        verifier: &str,
    ) -> Result<ProviderAccount, ProviderError> {
        let config = &state.config;
        let e = self.endpoints(config);
        let secret = self
            .secret(config)
            .ok_or(ProviderError::Missing("a client secret"))?;
        let redirect = self.redirect_uri(config);
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", self.settings(config).client_id.as_str()),
            ("client_secret", secret.expose()),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier),
        ];
        let token: TokenAnswer = state
            .http
            .post(&e.token_url)
            .header(ACCEPT, "application/json")
            .form(&form)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let access = token
            .access_token
            .ok_or(ProviderError::Missing("an access token"))?;

        match self {
            Provider::Google => {
                let u: GoogleUser = state
                    .http
                    .get(&e.user_url)
                    .bearer_auth(&access)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                Ok(ProviderAccount {
                    subject: u.sub,
                    email: crate::shared::validation::normalize_email(
                        &u.email.ok_or(ProviderError::Missing("an email"))?,
                    ),
                    email_verified: u.email_verified.unwrap_or(false),
                    first_name: u.given_name.filter(|s| !s.trim().is_empty()),
                    last_name: u.family_name.filter(|s| !s.trim().is_empty()),
                })
            }
            Provider::Github => {
                let get = |url: &str| {
                    state
                        .http
                        .get(url)
                        .bearer_auth(&access)
                        .header(USER_AGENT, "app-api")
                        .header(ACCEPT, "application/vnd.github+json")
                };
                let u: GithubUser = get(&e.user_url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                let emails_url = e
                    .emails_url
                    .ok_or(ProviderError::Missing("an emails endpoint"))?;
                let emails: Vec<GithubEmail> = get(&emails_url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                let primary = emails
                    .into_iter()
                    .find(|m| m.primary)
                    .ok_or(ProviderError::Missing("a primary email"))?;
                let (first_name, last_name) = split_name(u.name.as_deref());
                Ok(ProviderAccount {
                    subject: u.id.to_string(),
                    email: crate::shared::validation::normalize_email(&primary.email),
                    email_verified: primary.verified,
                    first_name,
                    last_name,
                })
            }
        }
    }
}

/// The providers this server offers, in the order the web shows them.
pub fn enabled(config: &Config) -> Vec<Provider> {
    Provider::ALL
        .into_iter()
        .filter(|p| p.enabled(config))
        .collect()
}

/// "Ana María López" → ("Ana María", "López"). A single word is a first name.
fn split_name(name: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) else {
        return (None, None);
    };
    match name.rsplit_once(' ') {
        Some((first, last)) => (Some(first.trim().to_string()), Some(last.to_string())),
        None => (Some(name.to_string()), None),
    }
}

#[derive(Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct GoogleUser {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
    given_name: Option<String>,
    family_name: Option<String>,
}

#[derive(Deserialize)]
struct GithubUser {
    id: u64,
    name: Option<String>,
}

#[derive(Deserialize)]
struct GithubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_at_the_last_space() {
        assert_eq!(
            split_name(Some(" Ana María López ")),
            (Some("Ana María".into()), Some("López".into()))
        );
        assert_eq!(split_name(Some("octocat")), (Some("octocat".into()), None));
        assert_eq!(split_name(Some("  ")), (None, None));
        assert_eq!(split_name(None), (None, None));
    }
}
