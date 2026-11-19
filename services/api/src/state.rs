use std::sync::Arc;

use app_core::Config;
use app_db::Pool;
use app_mail::Mailer;

use crate::auth::Passwords;
use crate::help::HelpDocs;

/// Shared by every handler. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Pool,
    pub help: Arc<HelpDocs>,
    pub mailer: Mailer,
    /// For calls to Google and GitHub.
    pub http: reqwest::Client,
    pub passwords: Passwords,
}

impl AppState {
    /// Mail goes through Resend when `RESEND_API_KEY` is set, to the log otherwise.
    pub fn new(config: Config, db: Pool) -> Self {
        let mailer = match &config.resend_api_key {
            Some(key) => {
                Mailer::resend(key.expose(), &config.settings.mail.from).unwrap_or_else(|e| {
                    tracing::error!(error = %e, "cannot set up Resend; mail goes to the log");
                    Mailer::Log
                })
            }
            None => Mailer::Log,
        };
        Self::with_mailer(config, db, mailer)
    }

    pub fn with_mailer(config: Config, db: Pool, mailer: Mailer) -> Self {
        let help = HelpDocs::new(config.settings.help.source, config.env);
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "cannot set up the HTTP client; using the default");
                reqwest::Client::new()
            });
        let passwords = Passwords::new(config.settings.password.max_concurrent_hashes);
        Self {
            passwords,
            config: Arc::new(config),
            db,
            help: Arc::new(help),
            mailer,
            http,
        }
    }
}
