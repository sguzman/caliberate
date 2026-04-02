//! Preferences system integration.

use caliberate_core::config::ControlPlane;
use eframe::egui;

pub struct PreferencesView {
    read_only_notice: String,
}

impl PreferencesView {
    pub fn new() -> Self {
        Self {
            read_only_notice: "Preferences are read-only in this build.".to_string(),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        ui.heading("Preferences");
        ui.separator();
        ui.label(&self.read_only_notice);
        ui.separator();

        egui::CollapsingHeader::new("App")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(format!("Name: {}", config.app.name));
                ui.label(format!("Environment: {}", config.app.environment));
                ui.label(format!("Mode: {:?}", config.app.mode));
                ui.label(format!("Instance ID: {}", config.app.instance_id));
            });

        egui::CollapsingHeader::new("Paths")
            .default_open(true)
            .show(ui, |ui| {
                ui.label(format!("Data: {}", config.paths.data_dir.display()));
                ui.label(format!("Cache: {}", config.paths.cache_dir.display()));
                ui.label(format!("Logs: {}", config.paths.log_dir.display()));
                ui.label(format!("Temp: {}", config.paths.tmp_dir.display()));
                ui.label(format!("Library: {}", config.paths.library_dir.display()));
            });

        egui::CollapsingHeader::new("Logging").show(ui, |ui| {
            ui.label(format!("Level: {}", config.logging.level));
            ui.label(format!("JSON: {}", config.logging.json));
            ui.label(format!("Stdout: {}", config.logging.stdout));
            ui.label(format!("File enabled: {}", config.logging.file_enabled));
            ui.label(format!(
                "File max size MB: {}",
                config.logging.file_max_size_mb
            ));
            ui.label(format!(
                "File max backups: {}",
                config.logging.file_max_backups
            ));
        });

        egui::CollapsingHeader::new("Database").show(ui, |ui| {
            ui.label(format!("SQLite: {}", config.db.sqlite_path.display()));
            ui.label(format!("Pool size: {}", config.db.pool_size));
            ui.label(format!("Busy timeout (ms): {}", config.db.busy_timeout_ms));
        });

        egui::CollapsingHeader::new("Server").show(ui, |ui| {
            ui.label(format!(
                "Host: {}:{} ({})",
                config.server.host, config.server.port, config.server.scheme
            ));
            ui.label(format!("URL prefix: {}", config.server.url_prefix));
            ui.label(format!("Auth enabled: {}", config.server.enable_auth));
            ui.label(format!(
                "Download enabled: {}",
                config.server.download_enabled
            ));
            ui.label(format!(
                "Download max bytes: {}",
                config.server.download_max_bytes
            ));
            ui.label(format!(
                "Allow external download: {}",
                config.server.download_allow_external
            ));
            ui.label(format!("API keys: {}", config.server.api_keys.len()));
        });

        egui::CollapsingHeader::new("Assets").show(ui, |ui| {
            ui.label(format!(
                "Compress raw assets: {}",
                config.assets.compress_raw_assets
            ));
            ui.label(format!(
                "Compress metadata DB: {}",
                config.assets.compress_metadata_db
            ));
            ui.label(format!("Hash algorithm: {}", config.assets.hash_algorithm));
            ui.label(format!("Hash on ingest: {}", config.assets.hash_on_ingest));
            ui.label(format!(
                "Verify checksum: {}",
                config.assets.verify_checksum
            ));
            ui.label(format!(
                "Compression level: {}",
                config.assets.compression_level
            ));
        });

        egui::CollapsingHeader::new("Ingest").show(ui, |ui| {
            ui.label(format!("Default mode: {:?}", config.ingest.default_mode));
            ui.label(format!(
                "Archive reference enabled: {}",
                config.ingest.archive_reference_enabled
            ));
            ui.label(format!(
                "Duplicate policy: {:?}",
                config.ingest.duplicate_policy
            ));
            ui.label(format!(
                "Duplicate identical policy: {:?}",
                config.ingest.duplicate_identical_policy
            ));
            ui.label(format!(
                "Duplicate compare mode: {:?}",
                config.ingest.duplicate_compare
            ));
            ui.label(format!(
                "Background ingest enabled: {}",
                config.ingest.background_enabled
            ));
            ui.label(format!(
                "Background workers: {}",
                config.ingest.background_workers
            ));
            ui.label(format!(
                "Background queue capacity: {}",
                config.ingest.background_queue_capacity
            ));
        });

        egui::CollapsingHeader::new("Conversion").show(ui, |ui| {
            ui.label(format!("Enabled: {}", config.conversion.enabled));
            ui.label(format!(
                "Allow passthrough: {}",
                config.conversion.allow_passthrough
            ));
            ui.label(format!(
                "Max input bytes: {}",
                config.conversion.max_input_bytes
            ));
            ui.label(format!(
                "Default output format: {}",
                config.conversion.default_output_format
            ));
            ui.label(format!(
                "Temp dir: {}",
                config.conversion.temp_dir.display()
            ));
            ui.label(format!(
                "Output dir: {}",
                config.conversion.output_dir.display()
            ));
        });

        egui::CollapsingHeader::new("FTS").show(ui, |ui| {
            ui.label(format!("Enabled: {}", config.fts.enabled));
            ui.label(format!("Tokenizer: {}", config.fts.tokenizer));
            ui.label(format!(
                "Rebuild on migrate: {}",
                config.fts.rebuild_on_migrate
            ));
            ui.label(format!("Min query len: {}", config.fts.min_query_len));
            ui.label(format!("Result limit: {}", config.fts.result_limit));
        });
    }
}
