use std::error::Error;
use std::fs;

use mamahjong_application::RegisterUser;
use mamahjong_server::{AppState, ServerConfig, build_router_with_web, spawn_sweeper};
use tokio::net::TcpListener;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

type AnyError = Box<dyn Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let config = ServerConfig::from_env()?;
    let _log_guards = init_telemetry(&config)?;

    let state = match config.administrator() {
        Some(administrator) => {
            let state =
                AppState::persistent_with_admin(config.data_dir(), administrator.cookie_secure())?;
            state.application().bootstrap_administrator(RegisterUser {
                login_name: administrator.login_name().to_owned(),
                password: administrator.password().to_owned(),
                nickname: administrator.nickname().to_owned(),
            })?;
            state
        }
        None => AppState::persistent(config.data_dir())?,
    };
    let app = build_router_with_web(state.clone(), config.admin_web_dir(), config.game_web_dir());
    let listener = TcpListener::bind(config.bind_address()).await?;
    let sweeper = spawn_sweeper(state.clone());

    state.readiness().mark_ready();
    info!(address = %config.bind_address(), "server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;
    sweeper.abort();
    info!("server stopped");

    Ok(())
}

fn init_telemetry(config: &ServerConfig) -> Result<Vec<WorkerGuard>, AnyError> {
    let filter = match std::env::var(EnvFilter::DEFAULT_ENV) {
        Ok(value) => EnvFilter::try_new(value)?,
        Err(std::env::VarError::NotPresent) => EnvFilter::try_new("info")?,
        Err(error) => return Err(Box::new(error)),
    };

    fs::create_dir_all(config.logs_dir())?;
    let file_appender = tracing_appender::rolling::daily(config.logs_dir(), "server.jsonl");
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);
    let (stdout_writer, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
    let stdout_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(stdout_writer)
        .with_filter(filter.clone());
    let file_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_filter(filter);
    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .try_init()?;
    Ok(vec![stdout_guard, file_guard])
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
