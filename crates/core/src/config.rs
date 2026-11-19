//! Configuration: secrets from the environment, settings from `config/app.toml`.
//!
//! Everything is validated once at start. A bad or missing value stops the process with a
//! message that names the variable or key, so a misconfigured server never half-starts.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where the settings file is read from when `APP_CONFIG` is not set.
pub const DEFAULT_CONFIG_PATH: &str = "config/app.toml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} is not set")]
    Missing(&'static str),
    #[error("{var} is invalid: {reason}")]
    Invalid { var: &'static str, reason: String },
    #[error("cannot read the settings file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("the settings file {path} is invalid: {reason}")]
    Parse { path: PathBuf, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AppEnv {
    Development,
    Production,
}

impl AppEnv {
    pub fn is_production(self) -> bool {
        self == AppEnv::Production
    }

    pub fn as_str(self) -> &'static str {
        match self {
            AppEnv::Development => "development",
            AppEnv::Production => "production",
        }
    }
}

/// A string that never shows up in `Debug` output or logs.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// The whole configuration of the API process.
#[derive(Debug, Clone)]
pub struct Config {
    pub env: AppEnv,
    pub database_url: Secret,
    pub api_bind: SocketAddr,
    pub sentry_dsn: Option<Secret>,
    /// Resend's API key. Without it, mail is written to the log (development only).
    pub resend_api_key: Option<Secret>,
    pub google_client_secret: Option<Secret>,
    pub github_client_secret: Option<Secret>,
    pub database: DatabasePool,
    pub settings: Settings,
}

/// How the API's database pool is sized. Per machine, so from the environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatabasePool {
    /// `DATABASE_MAX_CONNECTIONS`. Every instance's pool together must stay below Postgres'
    /// `max_connections`.
    pub max_connections: u32,
    /// `DATABASE_ACQUIRE_TIMEOUT_SECONDS`: how long a request waits for a free connection.
    pub acquire_timeout_seconds: u32,
    /// `DATABASE_STATEMENT_TIMEOUT_SECONDS`: Postgres ends a longer statement. 0 = no limit.
    pub statement_timeout_seconds: u32,
}

impl Default for DatabasePool {
    fn default() -> Self {
        Self {
            max_connections: 10,
            acquire_timeout_seconds: 5,
            statement_timeout_seconds: 30,
        }
    }
}

/// A whole number from the environment, at least `min`, or `default` when unset.
fn number(
    get: impl Fn(&str) -> Option<String>,
    var: &'static str,
    default: u32,
    min: u32,
) -> Result<u32, ConfigError> {
    let Some(raw) = get(var) else {
        return Ok(default);
    };
    match raw.trim().parse::<u32>() {
        Ok(n) if n >= min => Ok(n),
        _ => Err(ConfigError::Invalid {
            var,
            reason: format!("`{raw}` is not a whole number of at least {min}"),
        }),
    }
}

impl Config {
    /// Reads the environment (call `dotenvy` first) and the settings file.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// The same as [`Config::from_env`] with an injectable environment, for tests.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let get = |name: &str| get(name).filter(|v| !v.trim().is_empty());

        let env = match get("APP_ENV").as_deref() {
            None | Some("development") => AppEnv::Development,
            Some("production") => AppEnv::Production,
            Some(other) => {
                return Err(ConfigError::Invalid {
                    var: "APP_ENV",
                    reason: format!("`{other}` is not `development` or `production`"),
                });
            }
        };

        let database_url = get("DATABASE_URL").ok_or(ConfigError::Missing("DATABASE_URL"))?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err(ConfigError::Invalid {
                var: "DATABASE_URL",
                reason: "must start with postgres:// or postgresql://".into(),
            });
        }

        let api_bind: SocketAddr = get("API_BIND")
            .unwrap_or_else(|| "127.0.0.1:8080".into())
            .parse()
            .map_err(|e| ConfigError::Invalid {
                var: "API_BIND",
                reason: format!("{e}"),
            })?;
        if !api_bind.ip().is_loopback() && !env.is_production() {
            return Err(ConfigError::Invalid {
                var: "API_BIND",
                reason: "a non-loopback address is allowed only with APP_ENV=production".into(),
            });
        }

        let path = get("APP_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));
        let settings = Settings::load(&path)?;
        if env.is_production() {
            settings
                .check_production()
                .map_err(|reason| ConfigError::Parse {
                    path: path.clone(),
                    reason,
                })?;
        }

        let pool = DatabasePool::default();
        let database = DatabasePool {
            max_connections: number(get, "DATABASE_MAX_CONNECTIONS", pool.max_connections, 1)?,
            acquire_timeout_seconds: number(
                get,
                "DATABASE_ACQUIRE_TIMEOUT_SECONDS",
                pool.acquire_timeout_seconds,
                1,
            )?,
            statement_timeout_seconds: number(
                get,
                "DATABASE_STATEMENT_TIMEOUT_SECONDS",
                pool.statement_timeout_seconds,
                0,
            )?,
        };

        Ok(Self {
            env,
            database_url: Secret::new(database_url),
            api_bind,
            sentry_dsn: get("SENTRY_DSN").map(Secret::new),
            resend_api_key: get("RESEND_API_KEY").map(Secret::new),
            google_client_secret: get("GOOGLE_CLIENT_SECRET").map(Secret::new),
            github_client_secret: get("GITHUB_CLIENT_SECRET").map(Secret::new),
            database,
            settings,
        })
    }
}

/// The non-secret settings from `config/app.toml`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub app: AppSettings,
    pub session: SessionSettings,
    pub login_throttle: LoginThrottle,
    pub registration: Registration,
    pub help: HelpSettings,
    #[serde(default)]
    pub mail: MailSettings,
    #[serde(default)]
    pub login_code: LoginCodeSettings,
    #[serde(default)]
    pub wallet: WalletSettings,
    #[serde(default)]
    pub oauth: OauthSettings,
    #[serde(default)]
    pub password: PasswordSettings,
}

/// Password hashing. Each Argon2 call holds about 19 MiB while it runs.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct PasswordSettings {
    /// Hashes that may run at once; more wait their turn. 0 = twice the CPU cores.
    pub max_concurrent_hashes: u32,
}

/// Google and GitHub, the `external` sign-in method. A provider is on when its `client_id`
/// here and its secret in the environment are both set.
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct OauthSettings {
    /// The address people open the app on, with no trailing slash. The providers send people
    /// back to `{public_url}/api/auth/oauth/{provider}/callback`, which each provider's app
    /// must list.
    pub public_url: String,
    pub google: OauthProvider,
    pub github: OauthProvider,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct OauthProvider {
    pub client_id: String,
    /// Other endpoints than the provider's public ones (GitHub Enterprise, tests).
    pub endpoints: Option<OauthEndpoints>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OauthEndpoints {
    pub authorize_url: String,
    pub token_url: String,
    /// Google: the OpenID userinfo endpoint. GitHub: `/user`.
    pub user_url: String,
    /// GitHub only: `/user/emails`.
    pub emails_url: Option<String>,
}

/// Sign-In-With-Solana, the `wallet` sign-in method.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct WalletSettings {
    /// The host the web is served on, as the browser shows it (`app.example.com`, or
    /// `127.0.0.1:3000` in development, as `scripts/dev.sh` serves it). A signed message must name it, so a signature asked
    /// for by another site does not sign in here. Not read from the request: behind the proxy
    /// that is loopback.
    pub domain: String,
    /// The sentence the wallet shows above the signature request.
    pub statement: String,
    /// How long a challenge may wait to be signed.
    pub nonce_minutes: u32,
    /// Challenges one IP may ask for per `nonce_minutes`.
    pub max_nonces_per_ip: u32,
}

impl Default for WalletSettings {
    fn default() -> Self {
        Self {
            domain: "127.0.0.1:3000".into(),
            statement: "Sign in to App. This is a signature, not a transaction, and costs nothing."
                .into(),
            nonce_minutes: 5,
            max_nonces_per_ip: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MailSettings {
    /// The sender, `Name <address>`. The address's domain must be verified at Resend.
    pub from: String,
}

impl Default for MailSettings {
    fn default() -> Self {
        Self {
            from: "App <no-reply@example.com>".into(),
        }
    }
}

/// The 6-digit codes the `code` sign-in method mails.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct LoginCodeSettings {
    /// How long a code works.
    pub lifetime_minutes: u32,
    /// Wrong tries before the email is paused.
    pub max_attempts: u32,
    pub pause_minutes: u32,
    /// The least time between two mails to one address.
    pub resend_seconds: u32,
    /// Codes one IP may ask for, across addresses, per `lifetime_minutes`.
    pub max_requests_per_ip: u32,
}

impl Default for LoginCodeSettings {
    fn default() -> Self {
        Self {
            lifetime_minutes: 10,
            max_attempts: 3,
            pause_minutes: 15,
            resend_seconds: 60,
            max_requests_per_ip: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionSettings {
    pub cookie_name: String,
    pub lifetime_hours: u32,
    pub extend_after_hours: u32,
    /// How recent a sign-in must be to change the sign-in method. Older sessions sign in again.
    #[serde(default = "default_reauth_minutes")]
    pub reauth_minutes: u32,
}

fn default_reauth_minutes() -> u32 {
    10
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoginThrottle {
    /// Failed logins for one email inside the window before it is paused.
    pub max_failures: u32,
    /// Failed logins from one IP, across all emails, inside the window. Higher than the
    /// per-email limit because many people can share an IP.
    pub max_failures_per_ip: u32,
    pub window_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub mode: RegistrationMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationMode {
    /// New users are active at once.
    Open,
    /// New users wait as `pending` until an admin activates them.
    Approval,
    /// Only admins create users.
    Closed,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HelpSettings {
    pub source: HelpSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HelpSource {
    /// Disk in development, embedded in production.
    Auto,
    Disk,
    Embedded,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            app: AppSettings { name: "App".into() },
            session: SessionSettings {
                cookie_name: "app_session".into(),
                lifetime_hours: 168,
                extend_after_hours: 24,
                reauth_minutes: default_reauth_minutes(),
            },
            login_throttle: LoginThrottle {
                max_failures: 5,
                max_failures_per_ip: 20,
                window_minutes: 15,
            },
            registration: Registration {
                mode: RegistrationMode::Open,
            },
            help: HelpSettings {
                source: HelpSource::Auto,
            },
            mail: MailSettings::default(),
            login_code: LoginCodeSettings::default(),
            wallet: WalletSettings::default(),
            oauth: OauthSettings {
                public_url: "http://127.0.0.1:3000".into(),
                ..Default::default()
            },
            password: PasswordSettings::default(),
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text).map_err(|reason| ConfigError::Parse {
            path: path.to_path_buf(),
            reason,
        })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let settings: Settings = toml::from_str(text).map_err(|e| e.to_string())?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.app.name.trim().is_empty() {
            return Err("app.name must not be empty".into());
        }
        let cookie = &self.session.cookie_name;
        if cookie.is_empty()
            || !cookie
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("session.cookie_name may only hold letters, digits, `_` and `-`".into());
        }
        if self.session.lifetime_hours == 0 {
            return Err("session.lifetime_hours must be above 0".into());
        }
        if self.session.extend_after_hours >= self.session.lifetime_hours {
            return Err("session.extend_after_hours must be below session.lifetime_hours".into());
        }
        if self.session.reauth_minutes == 0 {
            return Err("session.reauth_minutes must be above 0".into());
        }
        if !self.mail.from.contains('@') {
            return Err("mail.from must hold an email address".into());
        }
        let c = &self.login_code;
        if c.lifetime_minutes == 0
            || c.max_attempts == 0
            || c.pause_minutes == 0
            || c.max_requests_per_ip == 0
        {
            return Err(
                "login_code.lifetime_minutes, max_attempts, pause_minutes and max_requests_per_ip \
                 must be above 0"
                    .into(),
            );
        }
        let w = &self.wallet;
        if w.domain.trim().is_empty() || w.domain.contains('/') || w.domain.contains(' ') {
            return Err("wallet.domain must be a host such as app.example.com".into());
        }
        if w.statement.contains('\n') {
            return Err("wallet.statement must be one line".into());
        }
        if w.nonce_minutes == 0 || w.max_nonces_per_ip == 0 {
            return Err("wallet.nonce_minutes and max_nonces_per_ip must be above 0".into());
        }
        let url = &self.oauth.public_url;
        let has_provider =
            !self.oauth.google.client_id.is_empty() || !self.oauth.github.client_id.is_empty();
        if has_provider
            && (!(url.starts_with("http://") || url.starts_with("https://")) || url.ends_with('/'))
        {
            return Err(
                "oauth.public_url must start with http:// or https:// and not end with /".into(),
            );
        }
        let t = &self.login_throttle;
        if t.max_failures == 0 || t.max_failures_per_ip == 0 || t.window_minutes == 0 {
            return Err(
                "login_throttle.max_failures, max_failures_per_ip and window_minutes must be above 0"
                    .into(),
            );
        }
        Ok(())
    }

    /// The addresses people reach a server on. The committed defaults point at a laptop, and a
    /// server left with them breaks wallet and provider sign-in in ways that are hard to trace.
    pub fn check_production(&self) -> Result<(), String> {
        if is_local_host(&self.wallet.domain) {
            return Err(format!(
                "wallet.domain is `{}`: set it to the host people open the app on",
                self.wallet.domain
            ));
        }
        let o = &self.oauth;
        let has_provider = !o.google.client_id.is_empty() || !o.github.client_id.is_empty();
        if has_provider {
            let host = o.public_url.strip_prefix("https://").ok_or_else(|| {
                format!(
                    "oauth.public_url is `{}`: it must start with https://",
                    o.public_url
                )
            })?;
            if is_local_host(host) {
                return Err(format!(
                    "oauth.public_url is `{}`: set it to the address people open the app on",
                    o.public_url
                ));
            }
        }
        Ok(())
    }
}

/// `host` or `host:port` on this machine.
fn is_local_host(host_port: &str) -> bool {
    let host = match host_port.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(rest),
        None => host_port.rsplit_once(':').map_or(host_port, |(h, _)| h),
    };
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || host.ends_with(".localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback() || ip.is_unspecified())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn settings_file() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/app.toml")
    }

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let mut map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        map.entry("APP_CONFIG".into())
            .or_insert_with(|| settings_file().display().to_string());
        move |k| map.get(k).cloned()
    }

    #[test]
    fn the_committed_settings_file_is_valid() {
        let s = Settings::load(&settings_file()).unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn missing_database_url_is_named() {
        let err = Config::from_lookup(lookup(&[])).unwrap_err();
        assert_eq!(err.to_string(), "DATABASE_URL is not set");
    }

    #[test]
    fn non_loopback_bind_needs_production() {
        let err = Config::from_lookup(lookup(&[
            ("DATABASE_URL", "postgres://u:p@localhost/app"),
            ("API_BIND", "0.0.0.0:8080"),
        ]))
        .unwrap_err();
        assert!(err.to_string().starts_with("API_BIND is invalid"));

        let file = production_settings_file("bind", "https://app.example.com");
        let ok = Config::from_lookup(lookup(&[
            ("DATABASE_URL", "postgres://u:p@localhost/app"),
            ("API_BIND", "0.0.0.0:8080"),
            ("APP_ENV", "production"),
            ("APP_CONFIG", file.as_str()),
        ]));
        assert!(ok.is_ok(), "{:?}", ok.err());
    }

    /// The committed settings file with the server's host, Google on and `oauth` as its
    /// public URL, written to a temp file.
    fn production_settings_file(tag: &str, oauth: &str) -> String {
        let text = std::fs::read_to_string(settings_file())
            .unwrap()
            .replace(
                "domain = \"127.0.0.1:3000\"",
                "domain = \"app.example.com\"",
            )
            .replace(
                "public_url = \"http://127.0.0.1:3000\"",
                &format!("public_url = \"{oauth}\""),
            )
            .replace(
                "[oauth.google]\nclient_id = \"\"",
                "[oauth.google]\nclient_id = \"id\"",
            );
        let path =
            std::env::temp_dir().join(format!("app-settings-{}-{tag}.toml", std::process::id()));
        std::fs::write(&path, text).unwrap();
        path.display().to_string()
    }

    #[test]
    fn production_refuses_the_laptop_addresses() {
        let url = ("DATABASE_URL", "postgres://u:p@localhost/app");
        let prod = ("APP_ENV", "production");
        let err = Config::from_lookup(lookup(&[url, prod])).unwrap_err();
        assert!(err.to_string().contains("wallet.domain"), "{err}");

        for (tag, public_url) in [
            ("http", "http://app.example.com"),
            ("loopback", "https://127.0.0.1:3000"),
            ("localhost", "https://localhost"),
        ] {
            let file = production_settings_file(tag, public_url);
            let err = Config::from_lookup(lookup(&[url, prod, ("APP_CONFIG", file.as_str())]))
                .unwrap_err();
            assert!(err.to_string().contains("oauth.public_url"), "{err}");
        }

        let file = production_settings_file("ok", "https://app.example.com");
        let ok = Config::from_lookup(lookup(&[url, prod, ("APP_CONFIG", file.as_str())]));
        assert!(ok.is_ok(), "{:?}", ok.err());
    }

    #[test]
    fn local_hosts() {
        for local in [
            "localhost",
            "localhost:3000",
            "app.localhost",
            "127.0.0.1:3000",
            "[::1]:3000",
            "0.0.0.0",
        ] {
            assert!(is_local_host(local), "{local}");
        }
        for public in ["app.example.com", "app.example.com:8443", "203.0.113.9"] {
            assert!(!is_local_host(public), "{public}");
        }
    }

    #[test]
    fn unknown_app_env_is_refused() {
        let err = Config::from_lookup(lookup(&[
            ("DATABASE_URL", "postgres://u:p@localhost/app"),
            ("APP_ENV", "staging"),
        ]))
        .unwrap_err();
        assert!(err.to_string().contains("APP_ENV"));
    }

    #[test]
    fn database_url_never_shows_in_debug() {
        let cfg = Config::from_lookup(lookup(&[(
            "DATABASE_URL",
            "postgres://u:hunter2@localhost/app",
        )]))
        .unwrap();
        assert!(!format!("{cfg:?}").contains("hunter2"));
    }

    #[test]
    fn unknown_keys_are_refused() {
        let text = std::fs::read_to_string(settings_file()).unwrap() + "\n[extra]\nx = 1\n";
        assert!(Settings::parse(&text).unwrap_err().contains("extra"));
    }

    #[test]
    fn pool_settings_have_defaults_and_refuse_bad_values() {
        let url = ("DATABASE_URL", "postgres://u:p@localhost/app");
        let cfg = Config::from_lookup(lookup(&[url])).unwrap();
        assert_eq!(cfg.database, DatabasePool::default());

        let cfg = Config::from_lookup(lookup(&[
            url,
            ("DATABASE_MAX_CONNECTIONS", "40"),
            ("DATABASE_STATEMENT_TIMEOUT_SECONDS", "0"),
        ]))
        .unwrap();
        assert_eq!(cfg.database.max_connections, 40);
        assert_eq!(cfg.database.statement_timeout_seconds, 0);

        for (var, bad) in [
            ("DATABASE_MAX_CONNECTIONS", "0"),
            ("DATABASE_MAX_CONNECTIONS", "ten"),
            ("DATABASE_ACQUIRE_TIMEOUT_SECONDS", "-1"),
        ] {
            let err = Config::from_lookup(lookup(&[url, (var, bad)])).unwrap_err();
            assert!(err.to_string().starts_with(var), "{err}");
        }
    }

    #[test]
    fn extend_must_be_below_lifetime() {
        let mut s = Settings::default();
        s.session.extend_after_hours = s.session.lifetime_hours;
        assert!(s.validate().is_err());
    }
}
