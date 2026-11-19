use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};

use app_api::{app, cli, server, state::AppState, telemetry};
use app_core::Config;

#[derive(Parser)]
#[command(
    name = "app-api",
    version,
    about = "The app's HTTP API and admin commands"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP server (the default).
    Serve,
    /// Print the OpenAPI document as JSON and exit.
    Openapi,
    /// Apply pending database migrations.
    Migrate {
        /// Revert the newest applied migration instead.
        #[arg(long)]
        revert_last: bool,
    },
    /// Create an admin account. Asks for the password.
    CreateAdmin {
        #[arg(long)]
        email: String,
        #[arg(long)]
        first_name: String,
        #[arg(long)]
        last_name: String,
        /// Read the password from the first line of stdin instead of the terminal.
        #[arg(long)]
        password_stdin: bool,
    },
    /// Set a new password for an account and sign it out everywhere. The account signs in with
    /// a password from then on, whatever method it used before. Asks for the password.
    ResetPassword {
        #[arg(long)]
        email: String,
        /// Read the password from the first line of stdin instead of the terminal.
        #[arg(long)]
        password_stdin: bool,
    },
}

fn main() -> ExitCode {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Serve);

    if let Command::Openapi = command {
        return match app::openapi().to_pretty_json() {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(e) => fail(&format!("cannot write the OpenAPI document: {e}")),
        };
    }

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => return fail(&e.to_string()),
    };

    if let Command::Serve = command {
        let sentry = match telemetry::init_sentry(config.sentry_dsn.as_ref(), config.env) {
            Ok(guard) => guard,
            Err(e) => return fail(&e),
        };
        telemetry::init_tracing(config.env, sentry.is_some());
        return run(async move {
            let db = connect(&config).await?;
            server::serve(AppState::new(config, db)).await
        });
    }

    run(async move {
        // A migration (an index built concurrently) may run longer than any request.
        let timeout_off = matches!(command, Command::Migrate { .. });
        let db = if timeout_off {
            connect_with(&config, None).await?
        } else {
            connect(&config).await?
        };
        match command {
            Command::Migrate { revert_last } => cli::migrate(&db, revert_last).await,
            Command::CreateAdmin {
                email,
                first_name,
                last_name,
                password_stdin,
            } => cli::create_admin(&db, &email, &first_name, &last_name, password_stdin).await,
            Command::ResetPassword {
                email,
                password_stdin,
            } => cli::reset_password(&db, &email, password_stdin).await,
            Command::Serve | Command::Openapi => Ok(()),
        }
    })
}

async fn connect(config: &Config) -> anyhow::Result<app_db::Pool> {
    let seconds = config.database.statement_timeout_seconds;
    let timeout = (seconds > 0).then(|| Duration::from_secs(u64::from(seconds)));
    connect_with(config, timeout).await
}

async fn connect_with(
    config: &Config,
    statement_timeout: Option<Duration>,
) -> anyhow::Result<app_db::Pool> {
    let settings = app_db::PoolSettings {
        max_connections: config.database.max_connections,
        acquire_timeout: Duration::from_secs(u64::from(config.database.acquire_timeout_seconds)),
        statement_timeout,
    };
    app_db::connect(config.database_url.expose(), settings)
        .await
        .context("cannot connect to the database (check DATABASE_URL)")
}

fn run(fut: impl Future<Output = anyhow::Result<()>>) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return fail(&format!("cannot start the runtime: {e}")),
    };
    match runtime.block_on(fut) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = ?e, "fatal");
            fail(&format!("{e:#}"))
        }
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}
