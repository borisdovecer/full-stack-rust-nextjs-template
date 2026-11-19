//! Runs the HTTP server until SIGINT or SIGTERM.

use anyhow::{Context, bail};
use tokio::net::TcpListener;

use crate::app;
use crate::state::AppState;

pub async fn serve(state: AppState) -> anyhow::Result<()> {
    let migrations = app_db::migration_state(&state.db)
        .await
        .context("cannot check the database migrations")?;
    if !migrations.is_current() {
        bail!("{}", migrations.describe());
    }

    let bind = state.config.api_bind;
    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("cannot listen on {bind}"))?;
    crate::jobs::housekeeping::spawn(state.db.clone());
    tracing::info!(%bind, env = state.config.env.as_str(), "listening");

    axum::serve(
        listener,
        app::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .context("server failed")?;

    tracing::info!("stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    tracing::info!("shutting down");
}
