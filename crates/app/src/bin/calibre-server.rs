use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use std::fs;

#[derive(Debug, Parser)]
#[command(name = "calibre-server", version, about = "Caliberate content server")]
struct ServerCli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: std::path::PathBuf,
    #[arg(long)]
    api_key: Option<String>,
    #[command(subcommand)]
    command: Option<ServerCommand>,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    CheckConfig,
    Health,
    OpdsRoot,
    OpdsBooks,
    OpdsSearch {
        #[arg(long)]
        query: String,
    },
    Download {
        #[arg(long)]
        id: i64,
        #[arg(long)]
        output: std::path::PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = ServerCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    let config = bootstrap.config;

    match &cli.command {
        Some(ServerCommand::CheckConfig) => {
            tracing::info!(component = "calibre-server", "configuration check passed");
            return Ok(());
        }
        Some(ServerCommand::Health) => {
            let client = build_client(&cli, &config)?;
            let url = build_url(&config, "/health");
            let body = client.get(url).send()?.text()?;
            println!("{body}");
            return Ok(());
        }
        Some(ServerCommand::OpdsRoot) => {
            let client = build_client(&cli, &config)?;
            let url = build_url(&config, "/opds");
            let body = client.get(url).send()?.text()?;
            println!("{body}");
            return Ok(());
        }
        Some(ServerCommand::OpdsBooks) => {
            let client = build_client(&cli, &config)?;
            let url = build_url(&config, "/opds/books");
            let body = client.get(url).send()?.text()?;
            println!("{body}");
            return Ok(());
        }
        Some(ServerCommand::OpdsSearch { query }) => {
            let client = build_client(&cli, &config)?;
            let url = build_url(
                &config,
                &format!("/opds/search?q={}", urlencoding::encode(query)),
            );
            let body = client.get(url).send()?.text()?;
            println!("{body}");
            return Ok(());
        }
        Some(ServerCommand::Download { id, output }) => {
            let client = build_client(&cli, &config)?;
            let url = build_url(&config, &format!("/opds/books/{id}/download"));
            let response = client.get(url).send()?;
            if !response.status().is_success() {
                return Err(format!("download failed: {}", response.status()).into());
            }
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = response.bytes()?;
            fs::write(output, &bytes)?;
            println!("Downloaded to {}", output.display());
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

fn build_client(
    cli: &ServerCli,
    config: &caliberate_core::config::ControlPlane,
) -> Result<Client, Box<dyn std::error::Error>> {
    let mut headers = HeaderMap::new();
    let api_key = cli
        .api_key
        .clone()
        .or_else(|| config.server.api_keys.first().cloned());
    if let Some(key) = api_key {
        let value = HeaderValue::from_str(&format!("Bearer {key}"))?;
        headers.insert(AUTHORIZATION, value);
    }
    Ok(Client::builder().default_headers(headers).build()?)
}

fn build_url(config: &caliberate_core::config::ControlPlane, path: &str) -> String {
    let mut base = format!(
        "{}://{}:{}",
        config.server.scheme, config.server.host, config.server.port
    );
    if !config.server.url_prefix.is_empty() {
        base.push_str(&config.server.url_prefix);
    }
    if path.starts_with('/') {
        format!("{base}{path}")
    } else {
        format!("{base}/{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::build_url;
    use caliberate_core::config::ControlPlane;
    use std::path::PathBuf;

    fn load_config() -> ControlPlane {
        let config_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/control-plane.toml");
        ControlPlane::load_from_path(&config_path).expect("load config")
    }

    #[test]
    fn builds_url_without_prefix() {
        let mut config = load_config();
        config.server.scheme = "http".to_string();
        config.server.host = "localhost".to_string();
        config.server.port = 8080;
        config.server.url_prefix.clear();
        let url = build_url(&config, "/health");
        assert_eq!(url, "http://localhost:8080/health");
    }

    #[test]
    fn builds_url_with_prefix() {
        let mut config = load_config();
        config.server.scheme = "https".to_string();
        config.server.host = "example.com".to_string();
        config.server.port = 443;
        config.server.url_prefix = "/api".to_string();
        let url = build_url(&config, "/opds");
        assert_eq!(url, "https://example.com:443/api/opds");
    }
}
