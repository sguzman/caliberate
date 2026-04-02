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
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    scheme: Option<String>,
    #[arg(long)]
    url_prefix: Option<String>,
    #[arg(long, default_value_t = false)]
    enable_auth: bool,
    #[arg(long, default_value_t = false)]
    disable_auth: bool,
    #[arg(long)]
    auth_mode: Option<String>,
    #[arg(long)]
    server_api_key: Vec<String>,
    #[arg(long, default_value_t = false)]
    clear_api_keys: bool,
    #[arg(long, default_value_t = false)]
    download_enabled: bool,
    #[arg(long, default_value_t = false)]
    download_disabled: bool,
    #[arg(long)]
    download_max_bytes: Option<u64>,
    #[arg(long, default_value_t = false)]
    download_allow_external: bool,
    #[arg(long, default_value_t = false)]
    download_disallow_external: bool,
    #[arg(long)]
    worker_threads: Option<usize>,
    #[arg(long)]
    max_blocking_threads: Option<usize>,
    #[arg(long)]
    shutdown_timeout_ms: Option<u64>,
    #[command(subcommand)]
    command: Option<ServerCommand>,
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    CheckConfig,
    Health,
    Users {
        #[command(subcommand)]
        command: UsersCommand,
    },
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

#[derive(Debug, Subcommand)]
enum UsersCommand {
    List,
    Add {
        #[arg(long)]
        key: String,
    },
    Remove {
        #[arg(long)]
        key: String,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = ServerCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    let mut config = bootstrap.config;
    apply_cli_overrides(&cli, &mut config)?;

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
        Some(ServerCommand::Users { command }) => {
            let mut config = config;
            match command {
                UsersCommand::List => {
                    if config.server.api_keys.is_empty() {
                        println!("No API keys configured");
                    } else {
                        for key in &config.server.api_keys {
                            println!("{key}");
                        }
                    }
                }
                UsersCommand::Add { key } => {
                    if config.server.api_keys.contains(key) {
                        println!("API key already exists");
                    } else {
                        config.server.api_keys.push(key.clone());
                        config.save_to_path(&cli.config)?;
                        println!("Added API key");
                    }
                }
                UsersCommand::Remove { key } => {
                    let before = config.server.api_keys.len();
                    config.server.api_keys.retain(|existing| existing != key);
                    if config.server.api_keys.len() == before {
                        println!("API key not found");
                    } else {
                        config.save_to_path(&cli.config)?;
                        println!("Removed API key");
                    }
                }
            }
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

fn apply_cli_overrides(
    cli: &ServerCli,
    config: &mut caliberate_core::config::ControlPlane,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(host) = &cli.host {
        config.server.host = host.clone();
    }
    if let Some(port) = cli.port {
        config.server.port = port;
    }
    if let Some(scheme) = &cli.scheme {
        config.server.scheme = scheme.clone();
    }
    if let Some(prefix) = &cli.url_prefix {
        config.server.url_prefix = normalize_prefix(prefix);
    }
    if cli.enable_auth && cli.disable_auth {
        return Err("cannot pass both --enable-auth and --disable-auth".into());
    }
    if cli.enable_auth {
        config.server.enable_auth = true;
    }
    if cli.disable_auth {
        config.server.enable_auth = false;
    }
    if let Some(mode) = &cli.auth_mode {
        config.server.auth_mode = match mode.as_str() {
            "bearer" => caliberate_core::config::ServerAuthMode::Bearer,
            _ => return Err(format!("unsupported auth mode: {mode}").into()),
        };
    }
    if cli.clear_api_keys {
        config.server.api_keys.clear();
    }
    if !cli.server_api_key.is_empty() {
        for key in &cli.server_api_key {
            if !config.server.api_keys.contains(key) {
                config.server.api_keys.push(key.clone());
            }
        }
    }
    if cli.download_enabled && cli.download_disabled {
        return Err("cannot pass both --download-enabled and --download-disabled".into());
    }
    if cli.download_enabled {
        config.server.download_enabled = true;
    }
    if cli.download_disabled {
        config.server.download_enabled = false;
    }
    if let Some(max_bytes) = cli.download_max_bytes {
        config.server.download_max_bytes = max_bytes;
    }
    if cli.download_allow_external && cli.download_disallow_external {
        return Err(
            "cannot pass both --download-allow-external and --download-disallow-external".into(),
        );
    }
    if cli.download_allow_external {
        config.server.download_allow_external = true;
    }
    if cli.download_disallow_external {
        config.server.download_allow_external = false;
    }
    if let Some(worker_threads) = cli.worker_threads {
        config.runtime.worker_threads = worker_threads;
    }
    if let Some(max_blocking_threads) = cli.max_blocking_threads {
        config.runtime.max_blocking_threads = max_blocking_threads;
    }
    if let Some(timeout) = cli.shutdown_timeout_ms {
        config.runtime.shutdown_timeout_ms = timeout;
    }
    Ok(())
}

fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut normalized = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    while normalized.ends_with('/') && normalized.len() > 1 {
        normalized.pop();
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{ServerCli, apply_cli_overrides, build_url, normalize_prefix};
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

    #[test]
    fn normalizes_url_prefix() {
        assert_eq!(normalize_prefix("api"), "/api");
        assert_eq!(normalize_prefix("/api/"), "/api");
        assert_eq!(normalize_prefix(""), "");
    }

    #[test]
    fn applies_host_port_scheme_overrides() {
        let cli = ServerCli {
            config: PathBuf::new(),
            api_key: None,
            host: Some("example.org".to_string()),
            port: Some(9090),
            scheme: Some("https".to_string()),
            url_prefix: None,
            enable_auth: false,
            disable_auth: false,
            auth_mode: None,
            server_api_key: Vec::new(),
            clear_api_keys: false,
            download_enabled: false,
            download_disabled: false,
            download_max_bytes: None,
            download_allow_external: false,
            download_disallow_external: false,
            worker_threads: None,
            max_blocking_threads: None,
            shutdown_timeout_ms: None,
            command: None,
        };
        let mut config = load_config();
        apply_cli_overrides(&cli, &mut config).expect("apply overrides");
        assert_eq!(config.server.host, "example.org");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.server.scheme, "https");
    }

    #[test]
    fn applies_url_prefix_override() {
        let cli = ServerCli {
            config: PathBuf::new(),
            api_key: None,
            host: None,
            port: None,
            scheme: None,
            url_prefix: Some("proxy".to_string()),
            enable_auth: false,
            disable_auth: false,
            auth_mode: None,
            server_api_key: Vec::new(),
            clear_api_keys: false,
            download_enabled: false,
            download_disabled: false,
            download_max_bytes: None,
            download_allow_external: false,
            download_disallow_external: false,
            worker_threads: None,
            max_blocking_threads: None,
            shutdown_timeout_ms: None,
            command: None,
        };
        let mut config = load_config();
        apply_cli_overrides(&cli, &mut config).expect("apply overrides");
        assert_eq!(config.server.url_prefix, "/proxy");
    }

    #[test]
    fn applies_auth_overrides() {
        let cli = ServerCli {
            config: PathBuf::new(),
            api_key: None,
            host: None,
            port: None,
            scheme: None,
            url_prefix: None,
            enable_auth: true,
            disable_auth: false,
            auth_mode: Some("bearer".to_string()),
            server_api_key: vec!["key123".to_string()],
            clear_api_keys: true,
            download_enabled: false,
            download_disabled: false,
            download_max_bytes: None,
            download_allow_external: false,
            download_disallow_external: false,
            worker_threads: None,
            max_blocking_threads: None,
            shutdown_timeout_ms: None,
            command: None,
        };
        let mut config = load_config();
        apply_cli_overrides(&cli, &mut config).expect("apply overrides");
        assert!(config.server.enable_auth);
        assert_eq!(config.server.api_keys, vec!["key123".to_string()]);
    }

    #[test]
    fn applies_download_overrides() {
        let cli = ServerCli {
            config: PathBuf::new(),
            api_key: None,
            host: None,
            port: None,
            scheme: None,
            url_prefix: None,
            enable_auth: false,
            disable_auth: false,
            auth_mode: None,
            server_api_key: Vec::new(),
            clear_api_keys: false,
            download_enabled: true,
            download_disabled: false,
            download_max_bytes: Some(1234),
            download_allow_external: true,
            download_disallow_external: false,
            worker_threads: None,
            max_blocking_threads: None,
            shutdown_timeout_ms: None,
            command: None,
        };
        let mut config = load_config();
        apply_cli_overrides(&cli, &mut config).expect("apply overrides");
        assert!(config.server.download_enabled);
        assert_eq!(config.server.download_max_bytes, 1234);
        assert!(config.server.download_allow_external);
    }

    #[test]
    fn applies_runtime_overrides() {
        let cli = ServerCli {
            config: PathBuf::new(),
            api_key: None,
            host: None,
            port: None,
            scheme: None,
            url_prefix: None,
            enable_auth: false,
            disable_auth: false,
            auth_mode: None,
            server_api_key: Vec::new(),
            clear_api_keys: false,
            download_enabled: false,
            download_disabled: false,
            download_max_bytes: None,
            download_allow_external: false,
            download_disallow_external: false,
            worker_threads: Some(4),
            max_blocking_threads: Some(8),
            shutdown_timeout_ms: Some(900),
            command: None,
        };
        let mut config = load_config();
        apply_cli_overrides(&cli, &mut config).expect("apply overrides");
        assert_eq!(config.runtime.worker_threads, 4);
        assert_eq!(config.runtime.max_blocking_threads, 8);
        assert_eq!(config.runtime.shutdown_timeout_ms, 900);
    }
}
