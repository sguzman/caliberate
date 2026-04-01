mod cli;
mod error;

use caliberate_app::bootstrap::init as bootstrap_init;
use caliberate_core::config::ControlPlane;
use clap::Parser;
use cli::{Cli, Command};
use error::{AppError, AppResult};
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let bootstrap = bootstrap_init(&cli.config)?;
    let config = bootstrap.config;

    tracing::info!(
        component = "app",
        config_path = %cli.config.display(),
        mode = ?config.app.mode,
        "caliberate startup complete"
    );

    let runtime = build_runtime(&config)?;
    runtime.block_on(async move { run(cli, config).await })?;

    Ok(())
}

fn build_runtime(config: &ControlPlane) -> AppResult<Runtime> {
    Builder::new_multi_thread()
        .worker_threads(config.runtime.worker_threads)
        .max_blocking_threads(config.runtime.max_blocking_threads)
        .enable_time()
        .enable_io()
        .build()
        .map_err(AppError::RuntimeInit)
}

async fn run(cli: Cli, config: ControlPlane) -> AppResult<()> {
    match cli.command {
        Some(Command::CheckConfig) => {
            tracing::info!(component = "cli", "configuration check passed");
            return Ok(());
        }
        None => {}
    }

    wait_for_shutdown(&config).await
}

async fn wait_for_shutdown(config: &ControlPlane) -> AppResult<()> {
    tracing::info!(component = "app", "waiting for shutdown signal");
    tokio::signal::ctrl_c().await.map_err(AppError::Signal)?;
    tracing::info!(component = "app", "shutdown signal received");

    let timeout = Duration::from_millis(config.runtime.shutdown_timeout_ms);
    tokio::time::timeout(timeout, async {
        tracing::info!(component = "app", "shutdown complete");
    })
    .await
    .map_err(|_| AppError::ShutdownTimeout(timeout))?;

    Ok(())
}
