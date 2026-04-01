use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "calibre-server", version, about = "Caliberate content server")]
struct ServerCli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: std::path::PathBuf,
    #[command(subcommand)]
    command: Option<ServerCommand>,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    CheckConfig,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = ServerCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    let config = bootstrap.config;

    match cli.command {
        Some(ServerCommand::CheckConfig) => {
            tracing::info!(component = "calibre-server", "configuration check passed");
            return Ok(());
        }
        None => {}
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime.worker_threads)
        .max_blocking_threads(config.runtime.max_blocking_threads)
        .enable_io()
        .enable_time()
        .build()?;

    runtime.block_on(async move { caliberate_server::run(&config).await })?;

    Ok(())
}
