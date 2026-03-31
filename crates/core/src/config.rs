//! Control-plane configuration loader.

use crate::error::{CoreError, CoreResult};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct ControlPlane {
    pub app: AppConfig,
    pub paths: PathsConfig,
    pub logging: LoggingConfig,
    pub db: DbConfig,
    pub runtime: RuntimeConfig,
    pub metrics: MetricsConfig,
    pub formats: FormatsConfig,
    pub ingest: IngestConfig,
    pub assets: AssetsConfig,
}

impl ControlPlane {
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let path_ref = path.as_ref();
        let content = fs::read_to_string(path_ref)
            .map_err(|err| CoreError::ConfigLoad(path_ref.to_path_buf(), err))?;
        let config: ControlPlane =
            toml::from_str(&content).map_err(|err| CoreError::ConfigParse(err.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> CoreResult<()> {
        if self.paths.log_dir.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "paths.log_dir must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub environment: String,
    #[serde(default)]
    pub mode: AppMode,
    pub instance_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AppMode {
    Dev,
    Prod,
}

impl Default for AppMode {
    fn default() -> Self {
        AppMode::Dev
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,
    #[serde(default = "default_tmp_dir")]
    pub tmp_dir: PathBuf,
    #[serde(default = "default_library_dir")]
    pub library_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default)]
    pub json: bool,
    #[serde(default = "default_stdout")]
    pub stdout: bool,
    #[serde(default)]
    pub file_enabled: bool,
    #[serde(default = "default_file_max_size_mb")]
    pub file_max_size_mb: u64,
    #[serde(default = "default_file_max_backups")]
    pub file_max_backups: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    pub sqlite_path: PathBuf,
    pub pool_size: u32,
    pub busy_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    pub worker_threads: usize,
    pub max_blocking_threads: usize,
    pub shutdown_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_metrics_namespace")]
    pub namespace: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormatsConfig {
    #[serde(default = "default_supported_formats")]
    pub supported: Vec<String>,
    #[serde(default = "default_archive_formats")]
    pub archive_formats: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestConfig {
    #[serde(default = "default_ingest_mode")]
    pub default_mode: IngestMode,
    #[serde(default = "default_archive_reference_enabled")]
    pub archive_reference_enabled: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IngestMode {
    Copy,
    Reference,
}

impl Default for IngestMode {
    fn default() -> Self {
        IngestMode::Copy
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetsConfig {
    #[serde(default = "default_compress_raw_assets")]
    pub compress_raw_assets: bool,
    #[serde(default)]
    pub compress_metadata_db: bool,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

fn default_cache_dir() -> PathBuf {
    PathBuf::from("./cache")
}

fn default_log_dir() -> PathBuf {
    PathBuf::from("./logs")
}

fn default_tmp_dir() -> PathBuf {
    PathBuf::from("./tmp")
}

fn default_library_dir() -> PathBuf {
    PathBuf::from("./library")
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_stdout() -> bool {
    true
}

fn default_file_max_size_mb() -> u64 {
    50
}

fn default_file_max_backups() -> u64 {
    5
}

fn default_metrics_namespace() -> String {
    "caliberate".to_string()
}

fn default_supported_formats() -> Vec<String> {
    vec![
        "epub".to_string(),
        "mobi".to_string(),
        "azw".to_string(),
        "azw3".to_string(),
        "pdf".to_string(),
    ]
}

fn default_archive_formats() -> Vec<String> {
    vec!["zip".to_string(), "rar".to_string(), "7z".to_string()]
}

fn default_ingest_mode() -> IngestMode {
    IngestMode::Copy
}

fn default_archive_reference_enabled() -> bool {
    true
}

fn default_compress_raw_assets() -> bool {
    true
}
