//! Headless Caliberate server entry point for Cargo installation and systemd.
//!
//! This delegates to the same HTTP server used by `calibre-server`, without
//! spawning a child process or depending on a checkout-local binary.
use caliberate_core::config::ControlPlane;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "caliberate-daemon",
    version,
    about = "Run the Caliberate library server in the foreground (systemd-ready)"
)]
struct Cli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: PathBuf,
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut config = ControlPlane::load_from_path(&cli.config)?;
    if let Some(host) = cli.host {
        if host.trim().is_empty() {
            return Err("--host must not be empty".into());
        }
        config.server.host = host;
    }
    if let Some(port) = cli.port {
        if port == 0 {
            return Err("--port must not be zero".into());
        }
        config.server.port = port;
    }

    // Keep the logging guard alive for the entire lifetime of the daemon.
    let _logging_guard = caliberate_core::logging::init(&config)?;
    caliberate_core::paths::ensure_runtime_paths(&config)?;
    let _metrics = caliberate_core::metrics::init(&config);
    tracing::info!(
        component = "caliberate-daemon",
        config_path = %cli.config.display(),
        host = %config.server.host,
        port = config.server.port,
        "starting Caliberate server"
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.runtime.worker_threads)
        .max_blocking_threads(config.runtime.max_blocking_threads)
        .enable_all()
        .build()?;
    runtime.block_on(caliberate_server::run(&config))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn parses_lan_launch_options() {
        let args = Cli::try_parse_from([
            "caliberate-daemon",
            "--config",
            "/home/example/.config/caliberate/control-plane.toml",
            "--host",
            "0.0.0.0",
            "--port",
            "8080",
        ])
        .expect("valid LAN CLI arguments");
        assert_eq!(args.host.as_deref(), Some("0.0.0.0"));
        assert_eq!(args.port, Some(8080));
        assert_eq!(
            args.config,
            std::path::PathBuf::from("/home/example/.config/caliberate/control-plane.toml")
        );
    }

    #[test]
    fn rejects_non_numeric_port() {
        assert!(Cli::try_parse_from(["caliberate-daemon", "--port", "bad"]).is_err());
    }
}
