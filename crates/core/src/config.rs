//! Control-plane configuration loader.

use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    pub conversion: ConversionConfig,
    pub fts: FtsConfig,
    pub device: DeviceConfig,
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub gui: GuiConfig,
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

    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> CoreResult<()> {
        self.validate()?;
        let content =
            toml::to_string_pretty(self).map_err(|err| CoreError::ConfigParse(err.to_string()))?;
        fs::write(path.as_ref(), content)
            .map_err(|err| CoreError::ConfigLoad(path.as_ref().to_path_buf(), err))?;
        Ok(())
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
        if self.server.scheme.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "server.scheme must not be empty".to_string(),
            ));
        }
        if self.server.scheme != "http" && self.server.scheme != "https" {
            return Err(CoreError::ConfigValidate(
                "server.scheme must be 'http' or 'https'".to_string(),
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
        if !matches!(
            self.fts.tokenizer.as_str(),
            "unicode61" | "unicode61 remove_diacritics 2"
        ) {
            return Err(CoreError::ConfigValidate(
                "fts.tokenizer must be 'unicode61' or 'unicode61 remove_diacritics 2'".to_string(),
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
        if self.conversion.max_input_bytes == 0 {
            return Err(CoreError::ConfigValidate(
                "conversion.max_input_bytes must be greater than 0".to_string(),
            ));
        }
        if self.conversion.default_output_format.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "conversion.default_output_format must not be empty".to_string(),
            ));
        }
        if self.device.mount_roots.is_empty() {
            return Err(CoreError::ConfigValidate(
                "device.mount_roots must not be empty".to_string(),
            ));
        }
        if self.device.library_subdir.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "device.library_subdir must not be empty".to_string(),
            ));
        }
        if self.plugins.plugins_dir.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "plugins.plugins_dir must not be empty".to_string(),
            ));
        }
        if !(40.0..=600.0).contains(&self.gui.table_row_height) {
            return Err(CoreError::ConfigValidate(
                "gui.table_row_height must be between 40 and 600".to_string(),
            ));
        }
        if self.gui.table_column_min_width <= 0.0 {
            return Err(CoreError::ConfigValidate(
                "gui.table_column_min_width must be greater than 0".to_string(),
            ));
        }
        if self.gui.table_column_max_width <= self.gui.table_column_min_width {
            return Err(CoreError::ConfigValidate(
                "gui.table_column_max_width must be greater than gui.table_column_min_width"
                    .to_string(),
            ));
        }
        if self.gui.cover_thumb_size <= 0.0 {
            return Err(CoreError::ConfigValidate(
                "gui.cover_thumb_size must be greater than 0".to_string(),
            ));
        }
        if self.gui.cover_preview_size <= 0.0 {
            return Err(CoreError::ConfigValidate(
                "gui.cover_preview_size must be greater than 0".to_string(),
            ));
        }
        if self.gui.toast_duration_secs <= 0.0 {
            return Err(CoreError::ConfigValidate(
                "gui.toast_duration_secs must be greater than 0".to_string(),
            ));
        }
        if self.gui.toast_max == 0 {
            return Err(CoreError::ConfigValidate(
                "gui.toast_max must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub name: String,
    pub environment: String,
    #[serde(default)]
    pub mode: AppMode,
    pub instance_id: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbConfig {
    pub sqlite_path: PathBuf,
    pub pool_size: u32,
    pub busy_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeConfig {
    pub worker_threads: usize,
    pub max_blocking_threads: usize,
    pub shutdown_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_server_host")]
    pub host: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
    #[serde(default = "default_server_scheme")]
    pub scheme: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_metrics_namespace")]
    pub namespace: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ServerAuthMode {
    Bearer,
}

impl Default for ServerAuthMode {
    fn default() -> Self {
        ServerAuthMode::Bearer
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FormatsConfig {
    #[serde(default = "default_supported_formats")]
    pub supported: Vec<String>,
    #[serde(default = "default_archive_formats")]
    pub archive_formats: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryConfig {
    #[serde(default = "default_library_delete_files")]
    pub delete_files_on_remove: bool,
    #[serde(default = "default_library_delete_reference_files")]
    pub delete_reference_files: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversionConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_conversion_allow_passthrough")]
    pub allow_passthrough: bool,
    #[serde(default = "default_conversion_max_input_bytes")]
    pub max_input_bytes: u64,
    #[serde(default = "default_conversion_default_output_format")]
    pub default_output_format: String,
    #[serde(default = "default_conversion_temp_dir")]
    pub temp_dir: PathBuf,
    #[serde(default = "default_conversion_output_dir")]
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeviceConfig {
    #[serde(default = "default_device_mount_roots")]
    pub mount_roots: Vec<PathBuf>,
    #[serde(default = "default_device_library_subdir")]
    pub library_subdir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginsConfig {
    #[serde(default = "default_plugins_enabled")]
    pub enabled: bool,
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GuiConfig {
    #[serde(default = "default_gui_list_view_mode")]
    pub list_view_mode: String,
    #[serde(default = "default_gui_table_row_height")]
    pub table_row_height: f32,
    #[serde(default = "default_gui_table_column_min_width")]
    pub table_column_min_width: f32,
    #[serde(default = "default_gui_table_column_max_width")]
    pub table_column_max_width: f32,
    #[serde(default = "default_gui_show_title")]
    pub show_title: bool,
    #[serde(default = "default_gui_show_authors")]
    pub show_authors: bool,
    #[serde(default = "default_gui_show_series")]
    pub show_series: bool,
    #[serde(default = "default_gui_show_tags")]
    pub show_tags: bool,
    #[serde(default = "default_gui_show_formats")]
    pub show_formats: bool,
    #[serde(default = "default_gui_show_rating")]
    pub show_rating: bool,
    #[serde(default = "default_gui_show_publisher")]
    pub show_publisher: bool,
    #[serde(default = "default_gui_show_languages")]
    pub show_languages: bool,
    #[serde(default = "default_gui_show_cover")]
    pub show_cover: bool,
    #[serde(default = "default_gui_col_width_title")]
    pub width_title: f32,
    #[serde(default = "default_gui_col_width_authors")]
    pub width_authors: f32,
    #[serde(default = "default_gui_col_width_series")]
    pub width_series: f32,
    #[serde(default = "default_gui_col_width_tags")]
    pub width_tags: f32,
    #[serde(default = "default_gui_col_width_formats")]
    pub width_formats: f32,
    #[serde(default = "default_gui_col_width_rating")]
    pub width_rating: f32,
    #[serde(default = "default_gui_col_width_publisher")]
    pub width_publisher: f32,
    #[serde(default = "default_gui_col_width_languages")]
    pub width_languages: f32,
    #[serde(default = "default_gui_col_width_cover")]
    pub width_cover: f32,
    #[serde(default = "default_gui_cover_thumb_size")]
    pub cover_thumb_size: f32,
    #[serde(default = "default_gui_cover_preview_size")]
    pub cover_preview_size: f32,
    #[serde(default = "default_gui_toast_duration_secs")]
    pub toast_duration_secs: f64,
    #[serde(default = "default_gui_toast_max")]
    pub toast_max: usize,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            list_view_mode: default_gui_list_view_mode(),
            table_row_height: default_gui_table_row_height(),
            table_column_min_width: default_gui_table_column_min_width(),
            table_column_max_width: default_gui_table_column_max_width(),
            show_title: default_gui_show_title(),
            show_authors: default_gui_show_authors(),
            show_series: default_gui_show_series(),
            show_tags: default_gui_show_tags(),
            show_formats: default_gui_show_formats(),
            show_rating: default_gui_show_rating(),
            show_publisher: default_gui_show_publisher(),
            show_languages: default_gui_show_languages(),
            show_cover: default_gui_show_cover(),
            width_title: default_gui_col_width_title(),
            width_authors: default_gui_col_width_authors(),
            width_series: default_gui_col_width_series(),
            width_tags: default_gui_col_width_tags(),
            width_formats: default_gui_col_width_formats(),
            width_rating: default_gui_col_width_rating(),
            width_publisher: default_gui_col_width_publisher(),
            width_languages: default_gui_col_width_languages(),
            width_cover: default_gui_col_width_cover(),
            cover_thumb_size: default_gui_cover_thumb_size(),
            cover_preview_size: default_gui_cover_preview_size(),
            toast_duration_secs: default_gui_toast_duration_secs(),
            toast_max: default_gui_toast_max(),
        }
    }
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

fn default_server_scheme() -> String {
    "http".to_string()
}

fn default_gui_list_view_mode() -> String {
    "table".to_string()
}

fn default_gui_table_row_height() -> f32 {
    48.0
}

fn default_gui_table_column_min_width() -> f32 {
    80.0
}

fn default_gui_table_column_max_width() -> f32 {
    520.0
}

fn default_gui_show_title() -> bool {
    true
}

fn default_gui_show_authors() -> bool {
    true
}

fn default_gui_show_series() -> bool {
    true
}

fn default_gui_show_tags() -> bool {
    true
}

fn default_gui_show_formats() -> bool {
    true
}

fn default_gui_show_rating() -> bool {
    true
}

fn default_gui_show_publisher() -> bool {
    true
}

    fn default_gui_show_languages() -> bool {
        true
    }

    fn default_gui_show_cover() -> bool {
        true
    }

fn default_gui_col_width_title() -> f32 {
    240.0
}

fn default_gui_col_width_authors() -> f32 {
    180.0
}

fn default_gui_col_width_series() -> f32 {
    140.0
}

fn default_gui_col_width_tags() -> f32 {
    180.0
}

fn default_gui_col_width_formats() -> f32 {
    120.0
}

fn default_gui_col_width_rating() -> f32 {
    90.0
}

fn default_gui_col_width_publisher() -> f32 {
    160.0
}

fn default_gui_col_width_languages() -> f32 {
    120.0
}

fn default_gui_col_width_cover() -> f32 {
    72.0
}

fn default_gui_cover_thumb_size() -> f32 {
    64.0
}

fn default_gui_cover_preview_size() -> f32 {
    200.0
}

fn default_gui_toast_duration_secs() -> f64 {
    6.0
}

fn default_gui_toast_max() -> usize {
    4
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
    "unicode61 remove_diacritics 2".to_string()
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

fn default_device_mount_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("./devices"),
        PathBuf::from("/media"),
        PathBuf::from("/run/media"),
    ]
}

fn default_device_library_subdir() -> String {
    "Caliberate Library".to_string()
}

fn default_plugins_enabled() -> bool {
    true
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from("./plugins")
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

fn default_conversion_allow_passthrough() -> bool {
    true
}

fn default_conversion_max_input_bytes() -> u64 {
    104_857_600
}

fn default_conversion_default_output_format() -> String {
    "epub".to_string()
}

fn default_conversion_temp_dir() -> PathBuf {
    PathBuf::from("./tmp/conversion")
}

fn default_conversion_output_dir() -> PathBuf {
    PathBuf::from("./output/conversion")
}
