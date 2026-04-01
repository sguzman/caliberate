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
    pub server: ServerConfig,
    pub metrics: MetricsConfig,
    pub formats: FormatsConfig,
    pub ingest: IngestConfig,
    pub assets: AssetsConfig,
    pub library: LibraryConfig,
    pub fts: FtsConfig,
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
        if self.server.host.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "server.host must not be empty".to_string(),
            ));
        }
        if self.server.port == 0 {
            return Err(CoreError::ConfigValidate(
                "server.port must be greater than 0".to_string(),
            ));
        }
        if !self.server.url_prefix.is_empty() && !self.server.url_prefix.starts_with('/') {
            return Err(CoreError::ConfigValidate(
                "server.url_prefix must start with '/'".to_string(),
            ));
        }
        if self.server.download_max_bytes == 0 {
            return Err(CoreError::ConfigValidate(
                "server.download_max_bytes must be greater than 0".to_string(),
            ));
        }
        if self.assets.hash_algorithm.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "assets.hash_algorithm must not be empty".to_string(),
            ));
        }
        if (self.assets.hash_on_ingest || self.assets.verify_checksum)
            && self.assets.hash_algorithm != "sha256"
        {
            return Err(CoreError::ConfigValidate(
                "assets.hash_algorithm must be 'sha256'".to_string(),
            ));
        }
        if !(1..=22).contains(&self.assets.compression_level) {
            return Err(CoreError::ConfigValidate(
                "assets.compression_level must be between 1 and 22".to_string(),
            ));
        }
        if self.fts.tokenizer.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "fts.tokenizer must not be empty".to_string(),
            ));
        }
        if self.fts.tokenizer != "unicode61" {
            return Err(CoreError::ConfigValidate(
                "fts.tokenizer must be 'unicode61'".to_string(),
            ));
        }
        if self.fts.min_query_len == 0 {
            return Err(CoreError::ConfigValidate(
                "fts.min_query_len must be greater than 0".to_string(),
            ));
        }
        if self.fts.result_limit == 0 {
            return Err(CoreError::ConfigValidate(
                "fts.result_limit must be greater than 0".to_string(),
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
pub struct ServerConfig {
    #[serde(default = "default_server_host")]
    pub host: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
    #[serde(default)]
    pub url_prefix: String,
    #[serde(default)]
    pub enable_auth: bool,
    #[serde(default = "default_server_auth_mode")]
    pub auth_mode: ServerAuthMode,
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default = "default_server_download_enabled")]
    pub download_enabled: bool,
    #[serde(default = "default_server_download_max_bytes")]
    pub download_max_bytes: u64,
    #[serde(default = "default_server_download_allow_external")]
    pub download_allow_external: bool,
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

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerAuthMode {
    Bearer,
}

impl Default for ServerAuthMode {
    fn default() -> Self {
        ServerAuthMode::Bearer
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormatsConfig {
    #[serde(default = "default_supported_formats")]
    pub supported: Vec<String>,
    #[serde(default = "default_archive_formats")]
    pub archive_formats: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LibraryConfig {
    #[serde(default = "default_library_delete_files")]
    pub delete_files_on_remove: bool,
    #[serde(default = "default_library_delete_reference_files")]
    pub delete_reference_files: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestConfig {
    #[serde(default = "default_ingest_mode")]
    pub default_mode: IngestMode,
    #[serde(default = "default_archive_reference_enabled")]
    pub archive_reference_enabled: bool,
    #[serde(default = "default_duplicate_policy")]
    pub duplicate_policy: DuplicatePolicy,
    #[serde(default = "default_duplicate_identical_policy")]
    pub duplicate_identical_policy: DuplicatePolicy,
    #[serde(default = "default_duplicate_compare")]
    pub duplicate_compare: DuplicateCompare,
    #[serde(default = "default_ingest_background_enabled")]
    pub background_enabled: bool,
    #[serde(default = "default_ingest_background_workers")]
    pub background_workers: usize,
    #[serde(default = "default_ingest_background_queue_capacity")]
    pub background_queue_capacity: usize,
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

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DuplicatePolicy {
    Error,
    Skip,
    Overwrite,
}

impl Default for DuplicatePolicy {
    fn default() -> Self {
        DuplicatePolicy::Error
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DuplicateCompare {
    Checksum,
    Size,
}

impl Default for DuplicateCompare {
    fn default() -> Self {
        DuplicateCompare::Checksum
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetsConfig {
    #[serde(default = "default_compress_raw_assets")]
    pub compress_raw_assets: bool,
    #[serde(default)]
    pub compress_metadata_db: bool,
    #[serde(default = "default_asset_hash_algorithm")]
    pub hash_algorithm: String,
    #[serde(default)]
    pub hash_on_ingest: bool,
    #[serde(default)]
    pub verify_checksum: bool,
    #[serde(default = "default_asset_compression_level")]
    pub compression_level: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FtsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_fts_tokenizer")]
    pub tokenizer: String,
    #[serde(default = "default_fts_rebuild_on_migrate")]
    pub rebuild_on_migrate: bool,
    #[serde(default = "default_fts_min_query_len")]
    pub min_query_len: usize,
    #[serde(default = "default_fts_result_limit")]
    pub result_limit: usize,
}

impl Default for FtsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tokenizer: default_fts_tokenizer(),
            rebuild_on_migrate: default_fts_rebuild_on_migrate(),
            min_query_len: default_fts_min_query_len(),
            result_limit: default_fts_result_limit(),
        }
    }
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

fn default_server_auth_mode() -> ServerAuthMode {
    ServerAuthMode::Bearer
}

fn default_server_download_enabled() -> bool {
    true
}

fn default_server_download_max_bytes() -> u64 {
    104_857_600
}

fn default_server_download_allow_external() -> bool {
    false
}

fn default_supported_formats() -> Vec<String> {
    vec![
        "epub".to_string(),
        "mobi".to_string(),
        "azw".to_string(),
        "azw3".to_string(),
        "pdf".to_string(),
        "docx".to_string(),
    ]
}

fn default_archive_formats() -> Vec<String> {
    vec![
        "zip".to_string(),
        "rar".to_string(),
        "7z".to_string(),
        "zpaq".to_string(),
    ]
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

fn default_asset_hash_algorithm() -> String {
    "sha256".to_string()
}

fn default_asset_compression_level() -> i32 {
    3
}

fn default_fts_tokenizer() -> String {
    "unicode61".to_string()
}

fn default_fts_rebuild_on_migrate() -> bool {
    true
}

fn default_fts_min_query_len() -> usize {
    2
}

fn default_fts_result_limit() -> usize {
    100
}

fn default_server_host() -> String {
    "127.0.0.1".to_string()
}

fn default_server_port() -> u16 {
    8080
}

fn default_duplicate_policy() -> DuplicatePolicy {
    DuplicatePolicy::Error
}

fn default_duplicate_identical_policy() -> DuplicatePolicy {
    DuplicatePolicy::Skip
}

fn default_duplicate_compare() -> DuplicateCompare {
    DuplicateCompare::Checksum
}

fn default_ingest_background_enabled() -> bool {
    false
}

fn default_ingest_background_workers() -> usize {
    2
}

fn default_ingest_background_queue_capacity() -> usize {
    64
}

fn default_library_delete_files() -> bool {
    false
}

fn default_library_delete_reference_files() -> bool {
    false
}
