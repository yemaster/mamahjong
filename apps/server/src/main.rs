use std::error::Error;

use mamahjong_server::{AppState, ServerConfig, build_router};
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

type AnyError = Box<dyn Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let config = ServerConfig::from_env()?;
    init_telemetry()?;

    let state = AppState::new();
    let app = build_router(state.clone());
    let listener = TcpListener::bind(config.bind_address()).await?;

    state.readiness().mark_ready();
    info!(address = %config.bind_address(), "server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;
    info!("server stopped");

    Ok(())
}

fn init_telemetry() -> Result<(), AnyError> {
    let filter = match std::env::var(EnvFilter::DEFAULT_ENV) {
        Ok(value) => EnvFilter::try_new(value)?,
        Err(std::env::VarError::NotPresent) => EnvFilter::try_new("info")?,
        Err(error) => return Err(Box::new(error)),
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .try_init()?;
    Ok(())
}

async fn shutdown_signal(state: AppState) {
    wait_for_shutdown_signal().await;
    state.readiness().mark_not_ready();
    info!("shutdown requested; draining connections");
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let ctrl_c = tokio::signal::ctrl_c();
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    tokio::select! {
        result = ctrl_c => {
            if let Err(error) = result {
                tracing::error!(%error, "failed to listen for Ctrl+C");
            }
        }
        () = terminate => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for Ctrl+C");
    }
}
