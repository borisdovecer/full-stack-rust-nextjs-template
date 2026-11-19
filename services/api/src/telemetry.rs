//! Logs and Sentry.
//!
//! Logs are human-readable in development and JSON in production. Sentry receives errors only:
//! `tracing::error!` events and panics. It never receives request bodies, headers, cookies or
//! user data, and it is off when `SENTRY_DSN` is empty.

use std::borrow::Cow;

use app_core::config::{AppEnv, Secret};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GIT_COMMIT: &str = env!("APP_GIT_COMMIT");

/// Starts Sentry. Keep the guard alive for the life of the process: dropping it flushes events.
pub fn init_sentry(
    dsn: Option<&Secret>,
    env: AppEnv,
) -> Result<Option<sentry::ClientInitGuard>, String> {
    let Some(dsn) = dsn else { return Ok(None) };
    let dsn: sentry::types::Dsn = dsn
        .expose()
        .parse()
        .map_err(|_| "SENTRY_DSN is invalid".to_string())?;

    let mut options = sentry::ClientOptions::new()
        .release(Cow::Owned(format!("app-api@{VERSION}+{GIT_COMMIT}")))
        .environment(env.as_str())
        .send_default_pii(false)
        .attach_stacktrace(true)
        .before_send(|mut event| {
            event.request = None;
            event.user = None;
            event.server_name = None;
            Some(event)
        });
    options.dsn = Some(dsn);
    Ok(Some(sentry::init(options)))
}

/// Installs the global subscriber. `with_sentry` adds the layer that forwards errors.
pub fn init_tracing(env: AppEnv, with_sentry: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info"));

    let fmt = match env {
        AppEnv::Production => tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .boxed(),
        AppEnv::Development => tracing_subscriber::fmt::layer().with_target(false).boxed(),
    };

    let sentry = with_sentry.then(|| {
        sentry::integrations::tracing::layer().event_filter(|meta| {
            use sentry::integrations::tracing::EventFilter;
            match *meta.level() {
                tracing::Level::ERROR => EventFilter::Event,
                tracing::Level::WARN | tracing::Level::INFO => EventFilter::Breadcrumb,
                _ => EventFilter::Ignore,
            }
        })
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt)
        .with(sentry)
        .init();
}
