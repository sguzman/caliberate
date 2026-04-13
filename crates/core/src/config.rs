//! Control-plane configuration loader.

use crate::error::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub network: NetworkConfig,
    #[serde(default)]
    pub metadata_download: MetadataDownloadConfig,
    #[serde(default)]
    pub news: NewsConfig,
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
        if self.server.tls_enabled
            && (self.server.tls_cert_path.as_os_str().is_empty()
                || self.server.tls_key_path.as_os_str().is_empty())
        {
            return Err(CoreError::ConfigValidate(
                "server.tls_cert_path and server.tls_key_path must be set when tls_enabled"
                    .to_string(),
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
        if self.conversion.input_profiles.is_empty() {
            return Err(CoreError::ConfigValidate(
                "conversion.input_profiles must not be empty".to_string(),
            ));
        }
        if self.conversion.output_profiles.is_empty() {
            return Err(CoreError::ConfigValidate(
                "conversion.output_profiles must not be empty".to_string(),
            ));
        }
        if self.conversion.default_input_profile.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "conversion.default_input_profile must not be empty".to_string(),
            ));
        }
        if self.conversion.default_output_profile.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "conversion.default_output_profile must not be empty".to_string(),
            ));
        }
        if !self
            .conversion
            .input_profiles
            .iter()
            .any(|profile| profile == &self.conversion.default_input_profile)
        {
            return Err(CoreError::ConfigValidate(
                "conversion.default_input_profile must exist in conversion.input_profiles"
                    .to_string(),
            ));
        }
        if !self
            .conversion
            .output_profiles
            .iter()
            .any(|profile| profile == &self.conversion.default_output_profile)
        {
            return Err(CoreError::ConfigValidate(
                "conversion.default_output_profile must exist in conversion.output_profiles"
                    .to_string(),
            ));
        }
        if self.conversion.page_margin_left < 0.0
            || self.conversion.page_margin_right < 0.0
            || self.conversion.page_margin_top < 0.0
            || self.conversion.page_margin_bottom < 0.0
        {
            return Err(CoreError::ConfigValidate(
                "conversion page margins must be >= 0".to_string(),
            ));
        }
        if !matches!(
            self.conversion.cover_policy.as_str(),
            "keep" | "replace" | "generate"
        ) {
            return Err(CoreError::ConfigValidate(
                "conversion.cover_policy must be one of keep/replace/generate".to_string(),
            ));
        }
        if self.conversion.save_to_disk_template.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "conversion.save_to_disk_template must not be empty".to_string(),
            ));
        }
        if !matches!(
            self.conversion.save_to_disk_conflict_policy.as_str(),
            "overwrite" | "skip" | "rename"
        ) {
            return Err(CoreError::ConfigValidate(
                "conversion.save_to_disk_conflict_policy must be one of overwrite/skip/rename"
                    .to_string(),
            ));
        }
        if self.conversion.max_job_history == 0 {
            return Err(CoreError::ConfigValidate(
                "conversion.max_job_history must be greater than 0".to_string(),
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
        if self.device.connection_timeout_ms < 100 {
            return Err(CoreError::ConfigValidate(
                "device.connection_timeout_ms must be at least 100".to_string(),
            ));
        }
        if !matches!(self.device.driver_backend.as_str(), "auto" | "usb" | "mtp") {
            return Err(CoreError::ConfigValidate(
                "device.driver_backend must be one of auto/usb/mtp".to_string(),
            ));
        }
        if self.news.retention_days == 0 {
            return Err(CoreError::ConfigValidate(
                "news.retention_days must be greater than 0".to_string(),
            ));
        }
        if self.news.fetch_limit == 0 {
            return Err(CoreError::ConfigValidate(
                "news.fetch_limit must be greater than 0".to_string(),
            ));
        }
        if self.plugins.plugins_dir.as_os_str().is_empty() {
            return Err(CoreError::ConfigValidate(
                "plugins.plugins_dir must not be empty".to_string(),
            ));
        }
        if !matches!(
            self.network.dns_mode.as_str(),
            "system" | "doh_cloudflare" | "doh_google"
        ) {
            return Err(CoreError::ConfigValidate(
                "network.dns_mode must be one of system/doh_cloudflare/doh_google".to_string(),
            ));
        }
        if self.metadata_download.timeout_ms < 100 {
            return Err(CoreError::ConfigValidate(
                "metadata_download.timeout_ms must be at least 100".to_string(),
            ));
        }
        if self.metadata_download.cover_max_bytes == 0 {
            return Err(CoreError::ConfigValidate(
                "metadata_download.cover_max_bytes must be greater than 0".to_string(),
            ));
        }
        if self.metadata_download.queue_batch_size == 0 {
            return Err(CoreError::ConfigValidate(
                "metadata_download.queue_batch_size must be greater than 0".to_string(),
            ));
        }
        if self.metadata_download.max_results_per_provider == 0 {
            return Err(CoreError::ConfigValidate(
                "metadata_download.max_results_per_provider must be greater than 0".to_string(),
            ));
        }
        if self.metadata_download.user_agent.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "metadata_download.user_agent must not be empty".to_string(),
            ));
        }
        if self
            .metadata_download
            .openlibrary_base_url
            .trim()
            .is_empty()
        {
            return Err(CoreError::ConfigValidate(
                "metadata_download.openlibrary_base_url must not be empty".to_string(),
            ));
        }
        if self
            .metadata_download
            .googlebooks_base_url
            .trim()
            .is_empty()
        {
            return Err(CoreError::ConfigValidate(
                "metadata_download.googlebooks_base_url must not be empty".to_string(),
            ));
        }
        if !self
            .metadata_download
            .providers
            .contains(&"openlibrary".to_string())
            && !self
                .metadata_download
                .providers
                .contains(&"googlebooks".to_string())
            && !self
                .metadata_download
                .providers
                .contains(&"amazon".to_string())
            && !self
                .metadata_download
                .providers
                .contains(&"isbndb".to_string())
        {
            return Err(CoreError::ConfigValidate(
                "metadata_download.providers must include at least one known provider".to_string(),
            ));
        }
        for provider in &self.metadata_download.providers {
            if !matches!(
                provider.as_str(),
                "openlibrary" | "googlebooks" | "amazon" | "isbndb"
            ) {
                return Err(CoreError::ConfigValidate(
                    "metadata_download.providers includes an unknown provider".to_string(),
                ));
            }
        }
        if !self.metadata_download.merge_tags_default
            && !self.metadata_download.merge_identifiers_default
            && !self.metadata_download.overwrite_title_default
            && !self.metadata_download.overwrite_authors_default
            && !self.metadata_download.overwrite_publisher_default
            && !self.metadata_download.overwrite_language_default
            && !self.metadata_download.overwrite_pubdate_default
            && !self.metadata_download.overwrite_comment_default
        {
            return Err(CoreError::ConfigValidate(
                "metadata_download merge/overwrite defaults cannot all be disabled".to_string(),
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
        if self.gui.cover_max_bytes == 0 {
            return Err(CoreError::ConfigValidate(
                "gui.cover_max_bytes must be greater than 0".to_string(),
            ));
        }
        if self.gui.width_date_added <= 0.0
            || self.gui.width_date_modified <= 0.0
            || self.gui.width_pubdate <= 0.0
        {
            return Err(CoreError::ConfigValidate(
                "gui date column widths must be greater than 0".to_string(),
            ));
        }
        if self.gui.reader_font_size <= 8.0 {
            return Err(CoreError::ConfigValidate(
                "gui.reader_font_size must be greater than 8".to_string(),
            ));
        }
        if self.gui.reader_line_spacing <= 1.0 {
            return Err(CoreError::ConfigValidate(
                "gui.reader_line_spacing must be greater than 1.0".to_string(),
            ));
        }
        if self.gui.reader_page_chars == 0 {
            return Err(CoreError::ConfigValidate(
                "gui.reader_page_chars must be greater than 0".to_string(),
            ));
        }
        if !matches!(self.gui.reader_theme.as_str(), "light" | "dark" | "sepia") {
            return Err(CoreError::ConfigValidate(
                "gui.reader_theme must be 'light', 'dark', or 'sepia'".to_string(),
            ));
        }
        if !matches!(self.gui.app_theme.as_str(), "system" | "light" | "dark") {
            return Err(CoreError::ConfigValidate(
                "gui.app_theme must be 'system', 'light', or 'dark'".to_string(),
            ));
        }
        if !matches!(
            self.gui.icon_set.as_str(),
            "calibre" | "outline" | "minimal"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.icon_set must be one of calibre/outline/minimal".to_string(),
            ));
        }
        if !matches!(
            self.gui.system_tray_mode.as_str(),
            "disabled" | "minimize_to_tray" | "close_to_tray"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.system_tray_mode must be one of disabled/minimize_to_tray/close_to_tray"
                    .to_string(),
            ));
        }
        if !matches!(
            self.gui.last_active_view.as_str(),
            "library" | "preferences"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.last_active_view must be 'library' or 'preferences'".to_string(),
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
        if !(1..=200).contains(&self.gui.search_history_max) {
            return Err(CoreError::ConfigValidate(
                "gui.search_history_max must be between 1 and 200".to_string(),
            ));
        }
        if !(1..=100).contains(&self.gui.stats_top_n) {
            return Err(CoreError::ConfigValidate(
                "gui.stats_top_n must be between 1 and 100".to_string(),
            ));
        }
        if !matches!(
            self.gui.group_mode.as_str(),
            "none" | "series" | "authors" | "tags"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.group_mode must be one of 'none', 'series', 'authors', or 'tags'".to_string(),
            ));
        }
        if !(1..=12).contains(&self.gui.shelf_columns) {
            return Err(CoreError::ConfigValidate(
                "gui.shelf_columns must be between 1 and 12".to_string(),
            ));
        }
        if !(0..=10).contains(&self.gui.low_rating_threshold) {
            return Err(CoreError::ConfigValidate(
                "gui.low_rating_threshold must be between 0 and 10".to_string(),
            ));
        }
        if !matches!(
            self.gui.global_search_scope.as_str(),
            "all" | "title" | "authors" | "tags" | "series"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.global_search_scope must be one of all/title/authors/tags/series".to_string(),
            ));
        }
        if !matches!(
            self.gui.shortcut_preset.as_str(),
            "default" | "calibre_like"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.shortcut_preset must be 'default' or 'calibre_like'".to_string(),
            ));
        }
        for action in &self.gui.toolbar_visible_actions {
            if action.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.toolbar_visible_actions entries must not be empty".to_string(),
                ));
            }
        }
        if self.gui.recent_libraries_max == 0 || self.gui.recent_libraries_max > 100 {
            return Err(CoreError::ConfigValidate(
                "gui.recent_libraries_max must be between 1 and 100".to_string(),
            ));
        }
        if self.gui.window_width < 640.0 || self.gui.window_height < 480.0 {
            return Err(CoreError::ConfigValidate(
                "gui.window_width/window_height are too small".to_string(),
            ));
        }
        if !matches!(self.gui.pane_browser_side.as_str(), "left" | "right") {
            return Err(CoreError::ConfigValidate(
                "gui.pane_browser_side must be 'left' or 'right'".to_string(),
            ));
        }
        if !matches!(self.gui.pane_details_side.as_str(), "left" | "right") {
            return Err(CoreError::ConfigValidate(
                "gui.pane_details_side must be 'left' or 'right'".to_string(),
            ));
        }
        if !(320.0..=2400.0).contains(&self.gui.pane_left_width) {
            return Err(CoreError::ConfigValidate(
                "gui.pane_left_width must be between 320 and 2400".to_string(),
            ));
        }
        if !(280.0..=2000.0).contains(&self.gui.pane_right_width) {
            return Err(CoreError::ConfigValidate(
                "gui.pane_right_width must be between 280 and 2000".to_string(),
            ));
        }
        if !matches!(
            self.gui.layout_preset.as_str(),
            "classic" | "focus" | "minimal" | "wide"
        ) {
            return Err(CoreError::ConfigValidate(
                "gui.layout_preset must be one of classic/focus/minimal/wide".to_string(),
            ));
        }
        for path in &self.gui.recent_libraries {
            if path.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.recent_libraries entries must not be empty".to_string(),
                ));
            }
        }
        if !matches!(self.gui.view_density.as_str(), "compact" | "comfortable") {
            return Err(CoreError::ConfigValidate(
                "gui.view_density must be 'compact' or 'comfortable'".to_string(),
            ));
        }
        for column in &self.gui.column_order {
            if column.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.column_order entries must not be empty".to_string(),
                ));
            }
        }
        for (name, preset) in &self.gui.sort_presets {
            if name.trim().is_empty() || preset.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.sort_presets keys and values must not be empty".to_string(),
                ));
            }
        }
        for (name, preset) in &self.gui.column_presets {
            if name.trim().is_empty() || preset.is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.column_presets keys must be non-empty and values must have entries"
                        .to_string(),
                ));
            }
        }
        if let Some(active) = &self.gui.active_sort_preset {
            if active.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.active_sort_preset must not be empty when set".to_string(),
                ));
            }
            if !self.gui.sort_presets.contains_key(active) {
                return Err(CoreError::ConfigValidate(
                    "gui.active_sort_preset must reference gui.sort_presets".to_string(),
                ));
            }
        }
        if let Some(active) = &self.gui.active_column_preset {
            if active.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.active_column_preset must not be empty when set".to_string(),
                ));
            }
            if !self.gui.column_presets.contains_key(active) {
                return Err(CoreError::ConfigValidate(
                    "gui.active_column_preset must reference gui.column_presets".to_string(),
                ));
            }
        }
        if let Some(active) = &self.gui.active_virtual_library {
            if active.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.active_virtual_library must not be empty when set".to_string(),
                ));
            }
        }
        for (name, filters) in &self.gui.virtual_library_filters {
            if name.trim().is_empty() {
                return Err(CoreError::ConfigValidate(
                    "gui.virtual_library_filters keys must not be empty".to_string(),
                ));
            }
            for filter in filters {
                if filter.trim().is_empty() {
                    return Err(CoreError::ConfigValidate(
                        "gui.virtual_library_filters entries must not be empty".to_string(),
                    ));
                }
            }
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
    #[serde(default)]
    pub tls_enabled: bool,
    #[serde(default)]
    pub tls_cert_path: PathBuf,
    #[serde(default)]
    pub tls_key_path: PathBuf,
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
    #[serde(default = "default_conversion_input_profiles")]
    pub input_profiles: Vec<String>,
    #[serde(default = "default_conversion_output_profiles")]
    pub output_profiles: Vec<String>,
    #[serde(default = "default_conversion_default_input_profile")]
    pub default_input_profile: String,
    #[serde(default = "default_conversion_default_output_profile")]
    pub default_output_profile: String,
    #[serde(default = "default_conversion_heuristic_enable")]
    pub heuristic_enable: bool,
    #[serde(default = "default_conversion_heuristic_unwrap_lines")]
    pub heuristic_unwrap_lines: bool,
    #[serde(default = "default_conversion_heuristic_delete_blank_lines")]
    pub heuristic_delete_blank_lines: bool,
    #[serde(default = "default_conversion_page_margin_left")]
    pub page_margin_left: f32,
    #[serde(default = "default_conversion_page_margin_right")]
    pub page_margin_right: f32,
    #[serde(default = "default_conversion_page_margin_top")]
    pub page_margin_top: f32,
    #[serde(default = "default_conversion_page_margin_bottom")]
    pub page_margin_bottom: f32,
    #[serde(default = "default_conversion_embed_fonts")]
    pub embed_fonts: bool,
    #[serde(default = "default_conversion_subset_fonts")]
    pub subset_fonts: bool,
    #[serde(default = "default_conversion_cover_policy")]
    pub cover_policy: String,
    #[serde(default = "default_conversion_warn_unsupported_options")]
    pub warn_unsupported_options: bool,
    #[serde(default = "default_conversion_save_to_disk_template")]
    pub save_to_disk_template: String,
    #[serde(default = "default_conversion_save_to_disk_conflict_policy")]
    pub save_to_disk_conflict_policy: String,
    #[serde(default)]
    pub save_to_disk_presets: BTreeMap<String, String>,
    #[serde(default = "default_conversion_temp_dir")]
    pub temp_dir: PathBuf,
    #[serde(default = "default_conversion_output_dir")]
    pub output_dir: PathBuf,
    #[serde(default = "default_conversion_job_history_path")]
    pub job_history_path: PathBuf,
    #[serde(default = "default_conversion_job_logs_dir")]
    pub job_logs_dir: PathBuf,
    #[serde(default = "default_conversion_max_job_history")]
    pub max_job_history: usize,
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
    #[serde(default = "default_device_send_auto_convert")]
    pub send_auto_convert: bool,
    #[serde(default = "default_device_send_overwrite")]
    pub send_overwrite: bool,
    #[serde(default = "default_device_sync_metadata")]
    pub sync_metadata: bool,
    #[serde(default = "default_device_sync_cover")]
    pub sync_cover: bool,
    #[serde(default = "default_device_scan_recursive")]
    pub scan_recursive: bool,
    #[serde(default = "default_device_driver_backend")]
    pub driver_backend: String,
    #[serde(default = "default_device_connection_timeout_ms")]
    pub connection_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NewsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_news_recipes_dir")]
    pub recipes_dir: PathBuf,
    #[serde(default = "default_news_downloads_dir")]
    pub downloads_dir: PathBuf,
    #[serde(default = "default_news_history_path")]
    pub history_path: PathBuf,
    #[serde(default = "default_news_retention_days")]
    pub retention_days: u64,
    #[serde(default = "default_news_auto_delete")]
    pub auto_delete: bool,
    #[serde(default = "default_news_fetch_limit")]
    pub fetch_limit: usize,
    #[serde(default = "default_news_source_enabled")]
    pub source_enabled: BTreeMap<String, bool>,
    #[serde(default = "default_news_source_schedule")]
    pub source_schedule: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PluginsConfig {
    #[serde(default = "default_plugins_enabled")]
    pub enabled: bool,
    #[serde(default = "default_plugins_dir")]
    pub plugins_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    #[serde(default)]
    pub http_proxy: String,
    #[serde(default)]
    pub https_proxy: String,
    #[serde(default)]
    pub no_proxy: String,
    #[serde(default)]
    pub offline_mode: bool,
    #[serde(default = "default_network_dns_mode")]
    pub dns_mode: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetadataDownloadConfig {
    #[serde(default = "default_metadata_download_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_metadata_download_user_agent")]
    pub user_agent: String,
    #[serde(default = "default_metadata_download_cover_max_bytes")]
    pub cover_max_bytes: usize,
    #[serde(default = "default_metadata_download_queue_batch_size")]
    pub queue_batch_size: usize,
    #[serde(default = "default_metadata_download_max_results_per_provider")]
    pub max_results_per_provider: usize,
    #[serde(default = "default_metadata_download_providers")]
    pub providers: Vec<String>,
    #[serde(default = "default_metadata_download_openlibrary_enabled")]
    pub openlibrary_enabled: bool,
    #[serde(default = "default_metadata_download_openlibrary_base_url")]
    pub openlibrary_base_url: String,
    #[serde(default = "default_metadata_download_googlebooks_enabled")]
    pub googlebooks_enabled: bool,
    #[serde(default = "default_metadata_download_googlebooks_base_url")]
    pub googlebooks_base_url: String,
    #[serde(default)]
    pub googlebooks_api_key: String,
    #[serde(default)]
    pub prefer_isbn_lookup: bool,
    #[serde(default = "default_metadata_download_merge_tags_default")]
    pub merge_tags_default: bool,
    #[serde(default = "default_metadata_download_merge_identifiers_default")]
    pub merge_identifiers_default: bool,
    #[serde(default = "default_metadata_download_overwrite_title_default")]
    pub overwrite_title_default: bool,
    #[serde(default = "default_metadata_download_overwrite_authors_default")]
    pub overwrite_authors_default: bool,
    #[serde(default = "default_metadata_download_overwrite_publisher_default")]
    pub overwrite_publisher_default: bool,
    #[serde(default = "default_metadata_download_overwrite_language_default")]
    pub overwrite_language_default: bool,
    #[serde(default = "default_metadata_download_overwrite_pubdate_default")]
    pub overwrite_pubdate_default: bool,
    #[serde(default = "default_metadata_download_overwrite_comment_default")]
    pub overwrite_comment_default: bool,
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
    #[serde(default = "default_gui_show_date_added")]
    pub show_date_added: bool,
    #[serde(default = "default_gui_show_date_modified")]
    pub show_date_modified: bool,
    #[serde(default = "default_gui_show_pubdate")]
    pub show_pubdate: bool,
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
    #[serde(default = "default_gui_col_width_date_added")]
    pub width_date_added: f32,
    #[serde(default = "default_gui_col_width_date_modified")]
    pub width_date_modified: f32,
    #[serde(default = "default_gui_col_width_pubdate")]
    pub width_pubdate: f32,
    #[serde(default = "default_gui_cover_thumb_size")]
    pub cover_thumb_size: f32,
    #[serde(default = "default_gui_cover_preview_size")]
    pub cover_preview_size: f32,
    #[serde(default = "default_gui_cover_dir")]
    pub cover_dir: PathBuf,
    #[serde(default = "default_gui_cover_cache_dir")]
    pub cover_cache_dir: PathBuf,
    #[serde(default = "default_gui_cover_max_bytes")]
    pub cover_max_bytes: u64,
    #[serde(default = "default_gui_reader_font_size")]
    pub reader_font_size: f32,
    #[serde(default = "default_gui_reader_line_spacing")]
    pub reader_line_spacing: f32,
    #[serde(default = "default_gui_reader_page_chars")]
    pub reader_page_chars: usize,
    #[serde(default = "default_gui_reader_theme")]
    pub reader_theme: String,
    #[serde(default = "default_gui_app_theme")]
    pub app_theme: String,
    #[serde(default = "default_gui_icon_set")]
    pub icon_set: String,
    #[serde(default = "default_gui_startup_open_last_library")]
    pub startup_open_last_library: bool,
    #[serde(default = "default_gui_startup_restore_tabs")]
    pub startup_restore_tabs: bool,
    #[serde(default = "default_gui_last_active_view")]
    pub last_active_view: String,
    #[serde(default = "default_gui_system_tray_mode")]
    pub system_tray_mode: String,
    #[serde(default = "default_gui_confirm_exit_with_jobs")]
    pub confirm_exit_with_jobs: bool,
    #[serde(default = "default_gui_toast_duration_secs")]
    pub toast_duration_secs: f64,
    #[serde(default = "default_gui_toast_max")]
    pub toast_max: usize,
    #[serde(default = "default_gui_search_history_max")]
    pub search_history_max: usize,
    #[serde(default = "default_gui_view_density")]
    pub view_density: String,
    #[serde(default = "default_gui_quick_details_panel")]
    pub quick_details_panel: bool,
    #[serde(default = "default_gui_show_format_badges")]
    pub show_format_badges: bool,
    #[serde(default = "default_gui_show_language_badges")]
    pub show_language_badges: bool,
    #[serde(default = "default_gui_column_order")]
    pub column_order: Vec<String>,
    #[serde(default)]
    pub sort_presets: BTreeMap<String, String>,
    #[serde(default)]
    pub active_sort_preset: Option<String>,
    #[serde(default = "default_gui_stats_top_n")]
    pub stats_top_n: usize,
    #[serde(default = "default_gui_group_mode")]
    pub group_mode: String,
    #[serde(default = "default_gui_shelf_columns")]
    pub shelf_columns: usize,
    #[serde(default = "default_gui_conditional_missing_cover")]
    pub conditional_missing_cover: bool,
    #[serde(default = "default_gui_conditional_low_rating")]
    pub conditional_low_rating: bool,
    #[serde(default = "default_gui_low_rating_threshold")]
    pub low_rating_threshold: i64,
    #[serde(default = "default_gui_color_missing_cover")]
    pub color_missing_cover: String,
    #[serde(default = "default_gui_color_low_rating")]
    pub color_low_rating: String,
    #[serde(default = "default_gui_toolbar_icon_only")]
    pub toolbar_icon_only: bool,
    #[serde(default = "default_gui_toolbar_visible_actions")]
    pub toolbar_visible_actions: Vec<String>,
    #[serde(default = "default_gui_global_search_scope")]
    pub global_search_scope: String,
    #[serde(default = "default_gui_shortcut_preset")]
    pub shortcut_preset: String,
    #[serde(default = "default_gui_command_palette_enabled")]
    pub command_palette_enabled: bool,
    #[serde(default = "default_gui_notification_center_enabled")]
    pub notification_center_enabled: bool,
    #[serde(default = "default_gui_drag_drop_hints")]
    pub drag_drop_hints: bool,
    #[serde(default = "default_gui_recent_libraries")]
    pub recent_libraries: Vec<String>,
    #[serde(default = "default_gui_recent_libraries_max")]
    pub recent_libraries_max: usize,
    #[serde(default = "default_gui_active_library_label")]
    pub active_library_label: String,
    #[serde(default = "default_gui_window_width")]
    pub window_width: f32,
    #[serde(default = "default_gui_window_height")]
    pub window_height: f32,
    #[serde(default = "default_gui_window_pos_x")]
    pub window_pos_x: f32,
    #[serde(default = "default_gui_window_pos_y")]
    pub window_pos_y: f32,
    #[serde(default = "default_gui_window_restore")]
    pub window_restore: bool,
    #[serde(default = "default_gui_mouse_gestures")]
    pub mouse_gestures: bool,
    #[serde(default = "default_gui_pane_browser_visible")]
    pub pane_browser_visible: bool,
    #[serde(default = "default_gui_pane_browser_side")]
    pub pane_browser_side: String,
    #[serde(default = "default_gui_pane_details_visible")]
    pub pane_details_visible: bool,
    #[serde(default = "default_gui_pane_details_side")]
    pub pane_details_side: String,
    #[serde(default = "default_gui_pane_jobs_visible")]
    pub pane_jobs_visible: bool,
    #[serde(default = "default_gui_pane_left_width")]
    pub pane_left_width: f32,
    #[serde(default = "default_gui_pane_right_width")]
    pub pane_right_width: f32,
    #[serde(default = "default_gui_layout_preset")]
    pub layout_preset: String,
    #[serde(default = "default_gui_user_manual_url")]
    pub user_manual_url: String,
    #[serde(default = "default_gui_project_home_url")]
    pub project_home_url: String,
    #[serde(default = "default_gui_report_issue_url")]
    pub report_issue_url: String,
    #[serde(default)]
    pub column_presets: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub active_column_preset: Option<String>,
    #[serde(default)]
    pub active_virtual_library: Option<String>,
    #[serde(default)]
    pub virtual_library_filters: BTreeMap<String, Vec<String>>,
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
            show_date_added: default_gui_show_date_added(),
            show_date_modified: default_gui_show_date_modified(),
            show_pubdate: default_gui_show_pubdate(),
            width_title: default_gui_col_width_title(),
            width_authors: default_gui_col_width_authors(),
            width_series: default_gui_col_width_series(),
            width_tags: default_gui_col_width_tags(),
            width_formats: default_gui_col_width_formats(),
            width_rating: default_gui_col_width_rating(),
            width_publisher: default_gui_col_width_publisher(),
            width_languages: default_gui_col_width_languages(),
            width_cover: default_gui_col_width_cover(),
            width_date_added: default_gui_col_width_date_added(),
            width_date_modified: default_gui_col_width_date_modified(),
            width_pubdate: default_gui_col_width_pubdate(),
            cover_thumb_size: default_gui_cover_thumb_size(),
            cover_preview_size: default_gui_cover_preview_size(),
            cover_dir: default_gui_cover_dir(),
            cover_cache_dir: default_gui_cover_cache_dir(),
            cover_max_bytes: default_gui_cover_max_bytes(),
            reader_font_size: default_gui_reader_font_size(),
            reader_line_spacing: default_gui_reader_line_spacing(),
            reader_page_chars: default_gui_reader_page_chars(),
            reader_theme: default_gui_reader_theme(),
            app_theme: default_gui_app_theme(),
            icon_set: default_gui_icon_set(),
            startup_open_last_library: default_gui_startup_open_last_library(),
            startup_restore_tabs: default_gui_startup_restore_tabs(),
            last_active_view: default_gui_last_active_view(),
            system_tray_mode: default_gui_system_tray_mode(),
            confirm_exit_with_jobs: default_gui_confirm_exit_with_jobs(),
            toast_duration_secs: default_gui_toast_duration_secs(),
            toast_max: default_gui_toast_max(),
            search_history_max: default_gui_search_history_max(),
            view_density: default_gui_view_density(),
            quick_details_panel: default_gui_quick_details_panel(),
            show_format_badges: default_gui_show_format_badges(),
            show_language_badges: default_gui_show_language_badges(),
            column_order: default_gui_column_order(),
            sort_presets: BTreeMap::new(),
            active_sort_preset: None,
            stats_top_n: default_gui_stats_top_n(),
            group_mode: default_gui_group_mode(),
            shelf_columns: default_gui_shelf_columns(),
            conditional_missing_cover: default_gui_conditional_missing_cover(),
            conditional_low_rating: default_gui_conditional_low_rating(),
            low_rating_threshold: default_gui_low_rating_threshold(),
            color_missing_cover: default_gui_color_missing_cover(),
            color_low_rating: default_gui_color_low_rating(),
            toolbar_icon_only: default_gui_toolbar_icon_only(),
            toolbar_visible_actions: default_gui_toolbar_visible_actions(),
            global_search_scope: default_gui_global_search_scope(),
            shortcut_preset: default_gui_shortcut_preset(),
            command_palette_enabled: default_gui_command_palette_enabled(),
            notification_center_enabled: default_gui_notification_center_enabled(),
            drag_drop_hints: default_gui_drag_drop_hints(),
            recent_libraries: default_gui_recent_libraries(),
            recent_libraries_max: default_gui_recent_libraries_max(),
            active_library_label: default_gui_active_library_label(),
            window_width: default_gui_window_width(),
            window_height: default_gui_window_height(),
            window_pos_x: default_gui_window_pos_x(),
            window_pos_y: default_gui_window_pos_y(),
            window_restore: default_gui_window_restore(),
            mouse_gestures: default_gui_mouse_gestures(),
            pane_browser_visible: default_gui_pane_browser_visible(),
            pane_browser_side: default_gui_pane_browser_side(),
            pane_details_visible: default_gui_pane_details_visible(),
            pane_details_side: default_gui_pane_details_side(),
            pane_jobs_visible: default_gui_pane_jobs_visible(),
            pane_left_width: default_gui_pane_left_width(),
            pane_right_width: default_gui_pane_right_width(),
            layout_preset: default_gui_layout_preset(),
            user_manual_url: default_gui_user_manual_url(),
            project_home_url: default_gui_project_home_url(),
            report_issue_url: default_gui_report_issue_url(),
            column_presets: BTreeMap::new(),
            active_column_preset: None,
            active_virtual_library: None,
            virtual_library_filters: BTreeMap::new(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            http_proxy: String::new(),
            https_proxy: String::new(),
            no_proxy: String::new(),
            offline_mode: false,
            dns_mode: default_network_dns_mode(),
        }
    }
}

impl Default for MetadataDownloadConfig {
    fn default() -> Self {
        Self {
            timeout_ms: default_metadata_download_timeout_ms(),
            user_agent: default_metadata_download_user_agent(),
            cover_max_bytes: default_metadata_download_cover_max_bytes(),
            queue_batch_size: default_metadata_download_queue_batch_size(),
            max_results_per_provider: default_metadata_download_max_results_per_provider(),
            providers: default_metadata_download_providers(),
            openlibrary_enabled: default_metadata_download_openlibrary_enabled(),
            openlibrary_base_url: default_metadata_download_openlibrary_base_url(),
            googlebooks_enabled: default_metadata_download_googlebooks_enabled(),
            googlebooks_base_url: default_metadata_download_googlebooks_base_url(),
            googlebooks_api_key: String::new(),
            prefer_isbn_lookup: false,
            merge_tags_default: default_metadata_download_merge_tags_default(),
            merge_identifiers_default: default_metadata_download_merge_identifiers_default(),
            overwrite_title_default: default_metadata_download_overwrite_title_default(),
            overwrite_authors_default: default_metadata_download_overwrite_authors_default(),
            overwrite_publisher_default: default_metadata_download_overwrite_publisher_default(),
            overwrite_language_default: default_metadata_download_overwrite_language_default(),
            overwrite_pubdate_default: default_metadata_download_overwrite_pubdate_default(),
            overwrite_comment_default: default_metadata_download_overwrite_comment_default(),
        }
    }
}

impl Default for NewsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            recipes_dir: default_news_recipes_dir(),
            downloads_dir: default_news_downloads_dir(),
            history_path: default_news_history_path(),
            retention_days: default_news_retention_days(),
            auto_delete: default_news_auto_delete(),
            fetch_limit: default_news_fetch_limit(),
            source_enabled: default_news_source_enabled(),
            source_schedule: default_news_source_schedule(),
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
    PathBuf::from("./.cache/caliberate/data")
}

fn default_cache_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/cache")
}

fn default_log_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/logs")
}

fn default_tmp_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/tmp")
}

fn default_library_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/library")
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

fn default_gui_show_date_added() -> bool {
    true
}

fn default_gui_show_date_modified() -> bool {
    true
}

fn default_gui_show_pubdate() -> bool {
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

fn default_gui_col_width_date_added() -> f32 {
    140.0
}

fn default_gui_col_width_date_modified() -> f32 {
    140.0
}

fn default_gui_col_width_pubdate() -> f32 {
    140.0
}

fn default_gui_cover_thumb_size() -> f32 {
    64.0
}

fn default_gui_cover_preview_size() -> f32 {
    200.0
}

fn default_gui_cover_dir() -> PathBuf {
    PathBuf::from("./data/covers")
}

fn default_gui_cover_cache_dir() -> PathBuf {
    PathBuf::from("./cache/covers")
}

fn default_gui_cover_max_bytes() -> u64 {
    10 * 1024 * 1024
}

fn default_gui_reader_font_size() -> f32 {
    16.0
}

fn default_gui_reader_line_spacing() -> f32 {
    1.4
}

fn default_gui_reader_page_chars() -> usize {
    1800
}

fn default_gui_reader_theme() -> String {
    "light".to_string()
}

fn default_gui_app_theme() -> String {
    "system".to_string()
}

fn default_gui_icon_set() -> String {
    "calibre".to_string()
}

fn default_gui_startup_open_last_library() -> bool {
    true
}

fn default_gui_startup_restore_tabs() -> bool {
    true
}

fn default_gui_last_active_view() -> String {
    "library".to_string()
}

fn default_gui_system_tray_mode() -> String {
    "disabled".to_string()
}

fn default_gui_confirm_exit_with_jobs() -> bool {
    true
}

fn default_gui_toast_duration_secs() -> f64 {
    6.0
}

fn default_gui_toast_max() -> usize {
    4
}

fn default_gui_search_history_max() -> usize {
    20
}

fn default_gui_view_density() -> String {
    "comfortable".to_string()
}

fn default_gui_quick_details_panel() -> bool {
    true
}

fn default_gui_show_format_badges() -> bool {
    true
}

fn default_gui_show_language_badges() -> bool {
    true
}

fn default_gui_column_order() -> Vec<String> {
    vec![
        "title".to_string(),
        "cover".to_string(),
        "authors".to_string(),
        "series".to_string(),
        "tags".to_string(),
        "formats".to_string(),
        "rating".to_string(),
        "publisher".to_string(),
        "languages".to_string(),
        "date_added".to_string(),
        "date_modified".to_string(),
        "pubdate".to_string(),
    ]
}

fn default_gui_stats_top_n() -> usize {
    8
}

fn default_gui_group_mode() -> String {
    "none".to_string()
}

fn default_gui_shelf_columns() -> usize {
    4
}

fn default_gui_conditional_missing_cover() -> bool {
    true
}

fn default_gui_conditional_low_rating() -> bool {
    true
}

fn default_gui_low_rating_threshold() -> i64 {
    4
}

fn default_gui_color_missing_cover() -> String {
    "#d4a017".to_string()
}

fn default_gui_color_low_rating() -> String {
    "#cc4444".to_string()
}

fn default_gui_toolbar_icon_only() -> bool {
    false
}

fn default_gui_toolbar_visible_actions() -> Vec<String> {
    vec![
        "add".to_string(),
        "remove".to_string(),
        "convert".to_string(),
        "save_to_disk".to_string(),
        "refresh".to_string(),
        "preferences".to_string(),
        "open_logs".to_string(),
    ]
}

fn default_gui_global_search_scope() -> String {
    "all".to_string()
}

fn default_gui_shortcut_preset() -> String {
    "default".to_string()
}

fn default_gui_command_palette_enabled() -> bool {
    true
}

fn default_gui_notification_center_enabled() -> bool {
    true
}

fn default_gui_drag_drop_hints() -> bool {
    true
}

fn default_gui_recent_libraries() -> Vec<String> {
    vec!["./.cache/caliberate/data/caliberate.db".to_string()]
}

fn default_gui_recent_libraries_max() -> usize {
    10
}

fn default_gui_active_library_label() -> String {
    "Default Library".to_string()
}

fn default_gui_window_width() -> f32 {
    1400.0
}

fn default_gui_window_height() -> f32 {
    900.0
}

fn default_gui_window_pos_x() -> f32 {
    40.0
}

fn default_gui_window_pos_y() -> f32 {
    40.0
}

fn default_gui_window_restore() -> bool {
    true
}

fn default_gui_mouse_gestures() -> bool {
    true
}

fn default_gui_pane_browser_visible() -> bool {
    true
}

fn default_gui_pane_browser_side() -> String {
    "left".to_string()
}

fn default_gui_pane_details_visible() -> bool {
    true
}

fn default_gui_pane_details_side() -> String {
    "right".to_string()
}

fn default_gui_pane_jobs_visible() -> bool {
    true
}

fn default_gui_pane_left_width() -> f32 {
    560.0
}

fn default_gui_pane_right_width() -> f32 {
    460.0
}

fn default_gui_layout_preset() -> String {
    "classic".to_string()
}

fn default_gui_user_manual_url() -> String {
    "https://github.com/sguzman/caliberate".to_string()
}

fn default_gui_project_home_url() -> String {
    "https://github.com/sguzman/caliberate".to_string()
}

fn default_gui_report_issue_url() -> String {
    "https://github.com/sguzman/caliberate/issues".to_string()
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
        PathBuf::from("./.cache/caliberate/devices"),
        PathBuf::from("/media"),
        PathBuf::from("/run/media"),
    ]
}

fn default_device_library_subdir() -> String {
    "Caliberate Library".to_string()
}

fn default_device_send_auto_convert() -> bool {
    false
}

fn default_device_send_overwrite() -> bool {
    false
}

fn default_device_sync_metadata() -> bool {
    true
}

fn default_device_sync_cover() -> bool {
    true
}

fn default_device_scan_recursive() -> bool {
    true
}

fn default_device_driver_backend() -> String {
    "auto".to_string()
}

fn default_device_connection_timeout_ms() -> u64 {
    5_000
}

fn default_news_recipes_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/news/recipes")
}

fn default_news_downloads_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/news/downloads")
}

fn default_news_history_path() -> PathBuf {
    PathBuf::from("./.cache/caliberate/news/history.log")
}

fn default_news_retention_days() -> u64 {
    30
}

fn default_news_auto_delete() -> bool {
    true
}

fn default_news_fetch_limit() -> usize {
    20
}

fn default_news_source_enabled() -> BTreeMap<String, bool> {
    BTreeMap::from([
        ("hacker-news".to_string(), true),
        ("lobsters".to_string(), true),
        ("project-gutenberg".to_string(), false),
    ])
}

fn default_news_source_schedule() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("hacker-news".to_string(), "hourly".to_string()),
        ("lobsters".to_string(), "daily@06:00".to_string()),
        (
            "project-gutenberg".to_string(),
            "weekly@sun-06:00".to_string(),
        ),
    ])
}

fn default_plugins_enabled() -> bool {
    true
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/plugins")
}

fn default_network_dns_mode() -> String {
    "system".to_string()
}

fn default_metadata_download_timeout_ms() -> u64 {
    7000
}

fn default_metadata_download_user_agent() -> String {
    "caliberate/0.1 (+https://example.invalid/caliberate)".to_string()
}

fn default_metadata_download_cover_max_bytes() -> usize {
    10 * 1024 * 1024
}

fn default_metadata_download_queue_batch_size() -> usize {
    32
}

fn default_metadata_download_max_results_per_provider() -> usize {
    5
}

fn default_metadata_download_providers() -> Vec<String> {
    vec![
        "openlibrary".to_string(),
        "googlebooks".to_string(),
        "amazon".to_string(),
        "isbndb".to_string(),
    ]
}

fn default_metadata_download_openlibrary_enabled() -> bool {
    true
}

fn default_metadata_download_openlibrary_base_url() -> String {
    "https://openlibrary.org".to_string()
}

fn default_metadata_download_googlebooks_enabled() -> bool {
    true
}

fn default_metadata_download_googlebooks_base_url() -> String {
    "https://www.googleapis.com/books/v1".to_string()
}

fn default_metadata_download_merge_tags_default() -> bool {
    true
}

fn default_metadata_download_merge_identifiers_default() -> bool {
    true
}

fn default_metadata_download_overwrite_title_default() -> bool {
    true
}

fn default_metadata_download_overwrite_authors_default() -> bool {
    true
}

fn default_metadata_download_overwrite_publisher_default() -> bool {
    false
}

fn default_metadata_download_overwrite_language_default() -> bool {
    false
}

fn default_metadata_download_overwrite_pubdate_default() -> bool {
    false
}

fn default_metadata_download_overwrite_comment_default() -> bool {
    false
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

fn default_conversion_input_profiles() -> Vec<String> {
    vec![
        "default".to_string(),
        "tablet".to_string(),
        "phone".to_string(),
        "kindle".to_string(),
    ]
}

fn default_conversion_output_profiles() -> Vec<String> {
    vec![
        "default".to_string(),
        "tablet".to_string(),
        "phone".to_string(),
        "kindle".to_string(),
    ]
}

fn default_conversion_default_input_profile() -> String {
    "default".to_string()
}

fn default_conversion_default_output_profile() -> String {
    "default".to_string()
}

fn default_conversion_heuristic_enable() -> bool {
    true
}

fn default_conversion_heuristic_unwrap_lines() -> bool {
    true
}

fn default_conversion_heuristic_delete_blank_lines() -> bool {
    false
}

fn default_conversion_page_margin_left() -> f32 {
    5.0
}

fn default_conversion_page_margin_right() -> f32 {
    5.0
}

fn default_conversion_page_margin_top() -> f32 {
    5.0
}

fn default_conversion_page_margin_bottom() -> f32 {
    5.0
}

fn default_conversion_embed_fonts() -> bool {
    false
}

fn default_conversion_subset_fonts() -> bool {
    true
}

fn default_conversion_cover_policy() -> String {
    "keep".to_string()
}

fn default_conversion_warn_unsupported_options() -> bool {
    true
}

fn default_conversion_save_to_disk_template() -> String {
    "{title}-{id}.{format}".to_string()
}

fn default_conversion_save_to_disk_conflict_policy() -> String {
    "rename".to_string()
}

fn default_conversion_temp_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/tmp/conversion")
}

fn default_conversion_output_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/output/conversion")
}

fn default_conversion_job_history_path() -> PathBuf {
    PathBuf::from("./.cache/caliberate/data/conversion-jobs.log")
}

fn default_conversion_job_logs_dir() -> PathBuf {
    PathBuf::from("./.cache/caliberate/logs/jobs")
}

fn default_conversion_max_job_history() -> usize {
    500
}
