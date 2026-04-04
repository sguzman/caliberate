//! Preferences system integration.

use caliberate_core::config::{ControlPlane, DuplicateCompare, DuplicatePolicy, IngestMode};
use caliberate_core::error::{CoreError, CoreResult};
use eframe::egui;
use tracing::info;

pub struct PreferencesView {
    read_only_notice: String,
    edit_mode: bool,
    state: PreferencesState,
    status: String,
    last_error: Option<String>,
    restart_required: bool,
    active_section: PrefSection,
}

impl PreferencesView {
    pub fn new(config: &ControlPlane) -> Self {
        Self {
            read_only_notice: "Preferences are read-only in this build.".to_string(),
            edit_mode: false,
            state: PreferencesState::from_config(config),
            status: "Ready".to_string(),
            last_error: None,
            restart_required: false,
            active_section: PrefSection::Behavior,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        config: &mut ControlPlane,
        config_path: &std::path::Path,
    ) -> CoreResult<()> {
        ui.heading("Preferences");
        ui.separator();
        let mut action = PrefAction::None;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.edit_mode, egui::Button::new("Edit"))
                .clicked()
            {
                action = PrefAction::BeginEdit;
            }
            if ui
                .add_enabled(self.edit_mode, egui::Button::new("Save"))
                .clicked()
            {
                action = PrefAction::Save;
            }
            if ui
                .add_enabled(self.edit_mode, egui::Button::new("Cancel"))
                .clicked()
            {
                action = PrefAction::Cancel;
            }
        });
        match action {
            PrefAction::BeginEdit => {
                self.state = PreferencesState::from_config(config);
                self.edit_mode = true;
                self.status = "Editing preferences".to_string();
            }
            PrefAction::Save => {
                let errors = self.validate_state();
                if !errors.is_empty() {
                    self.last_error = Some(errors.join("; "));
                    self.status = "Validation failed".to_string();
                    return Ok(());
                }
                let restart_needed = self.logging_changed(config);
                self.apply_state(config)?;
                config.save_to_path(config_path)?;
                self.edit_mode = false;
                self.restart_required = restart_needed;
                self.status = if restart_needed {
                    "Preferences saved (restart required)".to_string()
                } else {
                    "Preferences saved".to_string()
                };
                info!(
                    component = "gui",
                    path = %config_path.display(),
                    "preferences saved"
                );
            }
            PrefAction::Cancel => {
                self.state = PreferencesState::from_config(config);
                self.edit_mode = false;
                self.status = "Edit cancelled".to_string();
            }
            PrefAction::None => {}
        }
        ui.separator();
        if !self.edit_mode {
            ui.label(&self.read_only_notice);
            ui.separator();
        }
        if self.restart_required {
            ui.colored_label(
                egui::Color32::from_rgb(180, 110, 0),
                "Some changes require restart to take effect.",
            );
        }
        ui.separator();
        self.section_tabs(ui);
        ui.separator();
        match self.active_section {
            PrefSection::Behavior => self.render_behavior_section(ui, config),
            PrefSection::LookAndFeel => self.render_look_feel_section(ui, config),
            PrefSection::ImportExport => self.render_import_export_section(ui, config),
            PrefSection::Advanced => self.render_advanced_section(ui, config),
            PrefSection::System => self.render_system_section(ui, config),
        }

        Ok(())
    }

    pub fn status_line(&self) -> (&str, Option<&str>) {
        (self.status.as_str(), self.last_error.as_deref())
    }

    pub fn error_message(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn clear_error_message(&mut self) {
        self.last_error = None;
    }

    pub fn set_error(&mut self, message: String) {
        self.last_error = Some(message);
        self.status = "Error".to_string();
    }

    fn section_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for section in PrefSection::all() {
                if ui
                    .selectable_label(self.active_section == section, section.label())
                    .clicked()
                {
                    self.active_section = section;
                }
            }
        });
    }

    fn render_system_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
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
            if self.edit_mode {
                ui.text_edit_singleline(&mut self.state.logging_level);
                ui.checkbox(&mut self.state.logging_json, "JSON logging");
                ui.checkbox(&mut self.state.logging_stdout, "Stdout");
                ui.checkbox(&mut self.state.logging_file_enabled, "File enabled");
            } else {
                ui.label(format!("Level: {}", config.logging.level));
                ui.label(format!("JSON: {}", config.logging.json));
                ui.label(format!("Stdout: {}", config.logging.stdout));
                ui.label(format!("File enabled: {}", config.logging.file_enabled));
            }
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
    }

    fn render_behavior_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("Assets").show(ui, |ui| {
            if self.edit_mode {
                ui.checkbox(&mut self.state.assets_compress_raw, "Compress raw assets");
                ui.checkbox(
                    &mut self.state.assets_compress_metadata,
                    "Compress metadata DB",
                );
            } else {
                ui.label(format!(
                    "Compress raw assets: {}",
                    config.assets.compress_raw_assets
                ));
                ui.label(format!(
                    "Compress metadata DB: {}",
                    config.assets.compress_metadata_db
                ));
            }
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
            if self.edit_mode {
                ui.horizontal(|ui| {
                    ui.label("Default mode");
                    egui::ComboBox::from_id_salt("ingest_default_mode")
                        .selected_text(format!("{:?}", self.state.ingest_default_mode))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.state.ingest_default_mode,
                                IngestMode::Copy,
                                "Copy",
                            );
                            ui.selectable_value(
                                &mut self.state.ingest_default_mode,
                                IngestMode::Reference,
                                "Reference",
                            );
                        });
                });
                ui.checkbox(
                    &mut self.state.ingest_archive_reference_enabled,
                    "Archive reference enabled",
                );
                ui.horizontal(|ui| {
                    ui.label("Duplicate policy");
                    egui::ComboBox::from_id_salt("dup_policy")
                        .selected_text(format!("{:?}", self.state.ingest_duplicate_policy))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.state.ingest_duplicate_policy,
                                DuplicatePolicy::Error,
                                "Error",
                            );
                            ui.selectable_value(
                                &mut self.state.ingest_duplicate_policy,
                                DuplicatePolicy::Skip,
                                "Skip",
                            );
                            ui.selectable_value(
                                &mut self.state.ingest_duplicate_policy,
                                DuplicatePolicy::Overwrite,
                                "Overwrite",
                            );
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Duplicate compare mode");
                    egui::ComboBox::from_id_salt("dup_compare")
                        .selected_text(format!("{:?}", self.state.ingest_duplicate_compare))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.state.ingest_duplicate_compare,
                                DuplicateCompare::Checksum,
                                "Checksum",
                            );
                            ui.selectable_value(
                                &mut self.state.ingest_duplicate_compare,
                                DuplicateCompare::Size,
                                "Size",
                            );
                        });
                });
            } else {
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
            }
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

        egui::CollapsingHeader::new("Library").show(ui, |ui| {
            ui.label(format!(
                "Delete files on remove: {}",
                config.library.delete_files_on_remove
            ));
            ui.label(format!(
                "Delete reference files: {}",
                config.library.delete_reference_files
            ));
        });
    }

    fn render_look_feel_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("GUI").show(ui, |ui| {
            ui.label(format!("List view mode: {}", config.gui.list_view_mode));
            ui.label(format!("Row height: {}", config.gui.table_row_height));
            ui.label(format!("Columns visible:"));
            ui.label(format!("Title: {}", config.gui.show_title));
            ui.label(format!("Authors: {}", config.gui.show_authors));
            ui.label(format!("Series: {}", config.gui.show_series));
            ui.label(format!("Tags: {}", config.gui.show_tags));
            ui.label(format!("Formats: {}", config.gui.show_formats));
            ui.label(format!("Rating: {}", config.gui.show_rating));
            ui.label(format!("Publisher: {}", config.gui.show_publisher));
            ui.label(format!("Languages: {}", config.gui.show_languages));
            ui.label(format!("Cover: {}", config.gui.show_cover));
            ui.label(format!("Cover thumb size: {}", config.gui.cover_thumb_size));
            ui.label(format!(
                "Cover preview size: {}",
                config.gui.cover_preview_size
            ));
            ui.label(format!(
                "Toast duration: {}s",
                config.gui.toast_duration_secs
            ));
            ui.label(format!("Toast max: {}", config.gui.toast_max));
        });
        egui::CollapsingHeader::new("Reader").show(ui, |ui| {
            if self.edit_mode {
                ui.horizontal(|ui| {
                    ui.label("Font size");
                    ui.add(
                        egui::DragValue::new(&mut self.state.reader_font_size).range(10.0..=28.0),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Line spacing");
                    ui.add(
                        egui::DragValue::new(&mut self.state.reader_line_spacing)
                            .speed(0.05)
                            .range(1.1..=2.2),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Page chars");
                    ui.add(
                        egui::DragValue::new(&mut self.state.reader_page_chars).range(600..=6000),
                    );
                });
                ui.text_edit_singleline(&mut self.state.reader_theme);
            } else {
                ui.label(format!("Font size: {}", config.gui.reader_font_size));
                ui.label(format!("Line spacing: {}", config.gui.reader_line_spacing));
                ui.label(format!("Page chars: {}", config.gui.reader_page_chars));
                ui.label(format!("Theme: {}", config.gui.reader_theme));
            }
        });
    }

    fn render_import_export_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("Conversion").show(ui, |ui| {
            if self.edit_mode {
                ui.checkbox(&mut self.state.conversion_enabled, "Enabled");
                ui.checkbox(
                    &mut self.state.conversion_allow_passthrough,
                    "Allow passthrough",
                );
                ui.horizontal(|ui| {
                    ui.label("Max input bytes");
                    ui.add(egui::DragValue::new(
                        &mut self.state.conversion_max_input_bytes,
                    ));
                });
                ui.text_edit_singleline(&mut self.state.conversion_default_output_format);
                if self
                    .state
                    .conversion_default_output_format
                    .trim()
                    .is_empty()
                {
                    ui.colored_label(
                        egui::Color32::from_rgb(190, 0, 0),
                        "Default output format cannot be empty",
                    );
                }
            } else {
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
            }
            ui.label(format!(
                "Temp dir: {}",
                config.conversion.temp_dir.display()
            ));
            ui.label(format!(
                "Output dir: {}",
                config.conversion.output_dir.display()
            ));
        });

        egui::CollapsingHeader::new("Formats").show(ui, |ui| {
            ui.label(format!(
                "Supported formats: {}",
                config.formats.supported.join(", ")
            ));
            ui.label(format!(
                "Archive formats: {}",
                config.formats.archive_formats.join(", ")
            ));
        });
    }

    fn render_advanced_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("Server").show(ui, |ui| {
            if self.edit_mode {
                ui.text_edit_singleline(&mut self.state.server_host);
                if self.state.server_host.trim().is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(190, 0, 0), "Host cannot be empty");
                }
                ui.horizontal(|ui| {
                    ui.label("Port");
                    ui.add(egui::DragValue::new(&mut self.state.server_port).range(1..=65535));
                });
                ui.horizontal(|ui| {
                    ui.label("Scheme");
                    ui.text_edit_singleline(&mut self.state.server_scheme);
                });
                if !matches!(self.state.server_scheme.as_str(), "http" | "https") {
                    ui.colored_label(
                        egui::Color32::from_rgb(190, 0, 0),
                        "Scheme must be http or https",
                    );
                }
                ui.text_edit_singleline(&mut self.state.server_url_prefix);
                ui.checkbox(&mut self.state.server_enable_auth, "Auth enabled");
                ui.checkbox(&mut self.state.server_download_enabled, "Download enabled");
                ui.horizontal(|ui| {
                    ui.label("Download max bytes");
                    ui.add(egui::DragValue::new(
                        &mut self.state.server_download_max_bytes,
                    ));
                });
                ui.checkbox(
                    &mut self.state.server_download_allow_external,
                    "Allow external download",
                );
            } else {
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
            }
            ui.label(format!("API keys: {}", config.server.api_keys.len()));
        });

        egui::CollapsingHeader::new("FTS").show(ui, |ui| {
            if self.edit_mode {
                ui.checkbox(&mut self.state.fts_enabled, "Enabled");
                ui.text_edit_singleline(&mut self.state.fts_tokenizer);
                if self.state.fts_tokenizer.trim().is_empty() {
                    ui.colored_label(
                        egui::Color32::from_rgb(190, 0, 0),
                        "FTS tokenizer cannot be empty",
                    );
                }
                ui.checkbox(&mut self.state.fts_rebuild_on_migrate, "Rebuild on migrate");
                ui.horizontal(|ui| {
                    ui.label("Min query len");
                    ui.add(egui::DragValue::new(&mut self.state.fts_min_query_len).range(1..=20));
                });
                ui.horizontal(|ui| {
                    ui.label("Result limit");
                    ui.add(egui::DragValue::new(&mut self.state.fts_result_limit).range(1..=500));
                });
            } else {
                ui.label(format!("Enabled: {}", config.fts.enabled));
                ui.label(format!("Tokenizer: {}", config.fts.tokenizer));
                ui.label(format!(
                    "Rebuild on migrate: {}",
                    config.fts.rebuild_on_migrate
                ));
                ui.label(format!("Min query len: {}", config.fts.min_query_len));
                ui.label(format!("Result limit: {}", config.fts.result_limit));
            }
        });

        egui::CollapsingHeader::new("Metrics").show(ui, |ui| {
            ui.label(format!("Enabled: {}", config.metrics.enabled));
            ui.label(format!("Endpoint: {}", config.metrics.endpoint));
            ui.label(format!("Namespace: {}", config.metrics.namespace));
        });
    }

    fn apply_state(&mut self, config: &mut ControlPlane) -> CoreResult<()> {
        if self.state.logging_level.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "logging level cannot be empty".to_string(),
            ));
        }
        if self.state.server_scheme != "http" && self.state.server_scheme != "https" {
            return Err(CoreError::ConfigValidate(
                "server scheme must be http or https".to_string(),
            ));
        }
        if !self.state.server_url_prefix.is_empty()
            && !self.state.server_url_prefix.starts_with('/')
        {
            return Err(CoreError::ConfigValidate(
                "server url_prefix must start with '/'".to_string(),
            ));
        }
        if self
            .state
            .conversion_default_output_format
            .trim()
            .is_empty()
        {
            return Err(CoreError::ConfigValidate(
                "default output format cannot be empty".to_string(),
            ));
        }
        if self.state.fts_tokenizer.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "fts tokenizer cannot be empty".to_string(),
            ));
        }

        config.logging.level = self.state.logging_level.trim().to_string();
        config.logging.json = self.state.logging_json;
        config.logging.stdout = self.state.logging_stdout;
        config.logging.file_enabled = self.state.logging_file_enabled;
        config.server.host = self.state.server_host.trim().to_string();
        config.server.port = self.state.server_port;
        config.server.scheme = self.state.server_scheme.trim().to_string();
        config.server.url_prefix = self.state.server_url_prefix.trim().to_string();
        config.server.enable_auth = self.state.server_enable_auth;
        config.server.download_enabled = self.state.server_download_enabled;
        config.server.download_max_bytes = self.state.server_download_max_bytes;
        config.server.download_allow_external = self.state.server_download_allow_external;
        config.assets.compress_raw_assets = self.state.assets_compress_raw;
        config.assets.compress_metadata_db = self.state.assets_compress_metadata;
        config.ingest.default_mode = self.state.ingest_default_mode;
        config.ingest.archive_reference_enabled = self.state.ingest_archive_reference_enabled;
        config.ingest.duplicate_policy = self.state.ingest_duplicate_policy;
        config.ingest.duplicate_compare = self.state.ingest_duplicate_compare;
        config.conversion.enabled = self.state.conversion_enabled;
        config.conversion.allow_passthrough = self.state.conversion_allow_passthrough;
        config.conversion.max_input_bytes = self.state.conversion_max_input_bytes;
        config.conversion.default_output_format = self
            .state
            .conversion_default_output_format
            .trim()
            .to_string();
        config.fts.enabled = self.state.fts_enabled;
        config.fts.tokenizer = self.state.fts_tokenizer.trim().to_string();
        config.fts.rebuild_on_migrate = self.state.fts_rebuild_on_migrate;
        config.fts.min_query_len = self.state.fts_min_query_len;
        config.fts.result_limit = self.state.fts_result_limit;
        config.gui.reader_font_size = self.state.reader_font_size;
        config.gui.reader_line_spacing = self.state.reader_line_spacing;
        config.gui.reader_page_chars = self.state.reader_page_chars;
        config.gui.reader_theme = self.state.reader_theme.trim().to_string();

        Ok(())
    }

    fn logging_changed(&self, config: &ControlPlane) -> bool {
        self.state.logging_level.trim() != config.logging.level
            || self.state.logging_json != config.logging.json
            || self.state.logging_stdout != config.logging.stdout
            || self.state.logging_file_enabled != config.logging.file_enabled
    }

    fn validate_state(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.state.server_host.trim().is_empty() {
            errors.push("server host must not be empty".to_string());
        }
        if self.state.server_scheme.trim().is_empty() {
            errors.push("server scheme must not be empty".to_string());
        }
        if !matches!(self.state.server_scheme.as_str(), "http" | "https") {
            errors.push("server scheme must be http or https".to_string());
        }
        if self
            .state
            .conversion_default_output_format
            .trim()
            .is_empty()
        {
            errors.push("default output format must not be empty".to_string());
        }
        if self.state.fts_tokenizer.trim().is_empty() {
            errors.push("fts tokenizer must not be empty".to_string());
        }
        if !matches!(self.state.reader_theme.as_str(), "light" | "dark" | "sepia") {
            errors.push("reader theme must be light, dark, or sepia".to_string());
        }
        errors
    }
}

#[derive(Debug, Clone)]
struct PreferencesState {
    logging_level: String,
    logging_json: bool,
    logging_stdout: bool,
    logging_file_enabled: bool,
    server_host: String,
    server_port: u16,
    server_scheme: String,
    server_url_prefix: String,
    server_enable_auth: bool,
    server_download_enabled: bool,
    server_download_max_bytes: u64,
    server_download_allow_external: bool,
    assets_compress_raw: bool,
    assets_compress_metadata: bool,
    ingest_default_mode: IngestMode,
    ingest_archive_reference_enabled: bool,
    ingest_duplicate_policy: DuplicatePolicy,
    ingest_duplicate_compare: DuplicateCompare,
    conversion_enabled: bool,
    conversion_allow_passthrough: bool,
    conversion_max_input_bytes: u64,
    conversion_default_output_format: String,
    fts_enabled: bool,
    fts_tokenizer: String,
    fts_rebuild_on_migrate: bool,
    fts_min_query_len: usize,
    fts_result_limit: usize,
    reader_font_size: f32,
    reader_line_spacing: f32,
    reader_page_chars: usize,
    reader_theme: String,
}

impl PreferencesState {
    fn from_config(config: &ControlPlane) -> Self {
        Self {
            logging_level: config.logging.level.clone(),
            logging_json: config.logging.json,
            logging_stdout: config.logging.stdout,
            logging_file_enabled: config.logging.file_enabled,
            server_host: config.server.host.clone(),
            server_port: config.server.port,
            server_scheme: config.server.scheme.clone(),
            server_url_prefix: config.server.url_prefix.clone(),
            server_enable_auth: config.server.enable_auth,
            server_download_enabled: config.server.download_enabled,
            server_download_max_bytes: config.server.download_max_bytes,
            server_download_allow_external: config.server.download_allow_external,
            assets_compress_raw: config.assets.compress_raw_assets,
            assets_compress_metadata: config.assets.compress_metadata_db,
            ingest_default_mode: config.ingest.default_mode,
            ingest_archive_reference_enabled: config.ingest.archive_reference_enabled,
            ingest_duplicate_policy: config.ingest.duplicate_policy,
            ingest_duplicate_compare: config.ingest.duplicate_compare,
            conversion_enabled: config.conversion.enabled,
            conversion_allow_passthrough: config.conversion.allow_passthrough,
            conversion_max_input_bytes: config.conversion.max_input_bytes,
            conversion_default_output_format: config.conversion.default_output_format.clone(),
            fts_enabled: config.fts.enabled,
            fts_tokenizer: config.fts.tokenizer.clone(),
            fts_rebuild_on_migrate: config.fts.rebuild_on_migrate,
            fts_min_query_len: config.fts.min_query_len,
            fts_result_limit: config.fts.result_limit,
            reader_font_size: config.gui.reader_font_size,
            reader_line_spacing: config.gui.reader_line_spacing,
            reader_page_chars: config.gui.reader_page_chars,
            reader_theme: config.gui.reader_theme.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PrefAction {
    None,
    BeginEdit,
    Save,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrefSection {
    Behavior,
    LookAndFeel,
    ImportExport,
    Advanced,
    System,
}

impl PrefSection {
    fn label(&self) -> &'static str {
        match self {
            PrefSection::Behavior => "Behavior",
            PrefSection::LookAndFeel => "Look & Feel",
            PrefSection::ImportExport => "Import/Export",
            PrefSection::Advanced => "Advanced",
            PrefSection::System => "System",
        }
    }

    fn all() -> [PrefSection; 5] {
        [
            PrefSection::Behavior,
            PrefSection::LookAndFeel,
            PrefSection::ImportExport,
            PrefSection::Advanced,
            PrefSection::System,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::PreferencesState;
    use caliberate_core::config::ControlPlane;
    use std::path::PathBuf;

    fn load_config() -> ControlPlane {
        let config_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../config/control-plane.toml");
        ControlPlane::load_from_path(&config_path).expect("load config")
    }

    #[test]
    fn preferences_state_roundtrip() {
        let config = load_config();
        let state = PreferencesState::from_config(&config);
        assert_eq!(state.logging_level, config.logging.level);
        assert_eq!(state.server_host, config.server.host);
        assert_eq!(
            state.conversion_default_output_format,
            config.conversion.default_output_format
        );
    }
}
