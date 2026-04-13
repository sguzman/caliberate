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
    active_pane: PrefPane,
    section_filter_query: String,
    export_path_input: String,
    import_path_input: String,
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
            active_pane: PrefPane::BehaviorAssets,
            section_filter_query: String::new(),
            export_path_input: config
                .paths
                .tmp_dir
                .join("preferences-export.toml")
                .display()
                .to_string(),
            import_path_input: config
                .paths
                .tmp_dir
                .join("preferences-export.toml")
                .display()
                .to_string(),
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
        ui.columns(2, |columns| {
            columns[0].heading("Preferences Tree");
            self.preferences_tree(&mut columns[0]);
            columns[0].separator();
            self.section_tabs(&mut columns[0]);
            columns[0].horizontal(|ui| {
                ui.label("Search settings");
                ui.text_edit_singleline(&mut self.section_filter_query);
                if ui.button("Clear").clicked() {
                    self.section_filter_query.clear();
                }
            });
            columns[0].separator();
            columns[0].label(format!("Focused pane: {}", self.active_pane.label()));

            columns[1].horizontal(|ui| {
                if ui
                    .add_enabled(self.edit_mode, egui::Button::new("Reset section"))
                    .clicked()
                {
                    self.reset_active_section_to_defaults();
                    self.status = format!("Reset {} to defaults", self.active_section.label());
                    info!(
                        component = "gui_preferences",
                        section = self.active_section.label(),
                        "reset preferences section to defaults"
                    );
                }
            });
            columns[1].separator();
            match self.active_section {
                PrefSection::Behavior => self.render_behavior_section(&mut columns[1], config),
                PrefSection::LookAndFeel => self.render_look_feel_section(&mut columns[1], config),
                PrefSection::ImportExport => {
                    self.render_import_export_section(&mut columns[1], config)
                }
                PrefSection::Advanced => self.render_advanced_section(&mut columns[1], config),
                PrefSection::System => self.render_system_section(&mut columns[1], config),
            }
        });

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

    pub fn open_section_behavior(&mut self) {
        self.active_section = PrefSection::Behavior;
        self.active_pane = PrefPane::BehaviorAssets;
    }

    pub fn open_section_look_and_feel(&mut self) {
        self.active_section = PrefSection::LookAndFeel;
        self.active_pane = PrefPane::LookGui;
    }

    pub fn open_section_import_export(&mut self) {
        self.active_section = PrefSection::ImportExport;
        self.active_pane = PrefPane::ImportExportPrefs;
    }

    pub fn open_section_advanced(&mut self) {
        self.active_section = PrefSection::Advanced;
        self.active_pane = PrefPane::AdvancedServer;
    }

    pub fn open_section_system(&mut self) {
        self.active_section = PrefSection::System;
        self.active_pane = PrefPane::SystemApp;
    }

    fn section_tabs(&mut self, ui: &mut egui::Ui) {
        let query = self.section_filter_query.trim().to_lowercase();
        ui.horizontal(|ui| {
            for section in PrefSection::all() {
                if !query.is_empty() && !section.label().to_lowercase().contains(&query) {
                    continue;
                }
                if ui
                    .selectable_label(self.active_section == section, section.label())
                    .clicked()
                {
                    self.active_section = section;
                    if let Some(first) = PrefPane::for_section(section).first().copied() {
                        self.active_pane = first;
                    }
                }
            }
        });
    }

    fn preferences_tree(&mut self, ui: &mut egui::Ui) {
        let query = self.section_filter_query.trim().to_lowercase();
        for section in PrefSection::all() {
            let panes = PrefPane::for_section(section);
            let matches_section = query.is_empty()
                || section.label().to_lowercase().contains(&query)
                || panes
                    .iter()
                    .any(|pane| pane.label().to_lowercase().contains(&query));
            if !matches_section {
                continue;
            }
            ui.collapsing(section.label(), |ui| {
                for pane in panes {
                    if !query.is_empty() && !pane.label().to_lowercase().contains(&query) {
                        continue;
                    }
                    if ui
                        .selectable_label(self.active_pane == *pane, pane.label())
                        .clicked()
                    {
                        self.active_pane = *pane;
                        self.active_section = pane.section();
                    }
                }
            });
        }
    }

    fn render_system_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("App")
            .default_open(matches!(self.active_pane, PrefPane::SystemApp))
            .show(ui, |ui| {
                ui.label(format!("Name: {}", config.app.name));
                ui.label(format!("Environment: {}", config.app.environment));
                ui.label(format!("Mode: {:?}", config.app.mode));
                ui.label(format!("Instance ID: {}", config.app.instance_id));
            });

        egui::CollapsingHeader::new("Paths")
            .default_open(matches!(self.active_pane, PrefPane::SystemPaths))
            .show(ui, |ui| {
                if self.edit_mode {
                    ui.horizontal(|ui| {
                        ui.label("Data");
                        ui.text_edit_singleline(&mut self.state.path_data_dir);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cache");
                        ui.text_edit_singleline(&mut self.state.path_cache_dir);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Logs");
                        ui.text_edit_singleline(&mut self.state.path_log_dir);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Temp");
                        ui.text_edit_singleline(&mut self.state.path_tmp_dir);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Library");
                        ui.text_edit_singleline(&mut self.state.path_library_dir);
                    });
                } else {
                    ui.label(format!("Data: {}", config.paths.data_dir.display()));
                    ui.label(format!("Cache: {}", config.paths.cache_dir.display()));
                    ui.label(format!("Logs: {}", config.paths.log_dir.display()));
                    ui.label(format!("Temp: {}", config.paths.tmp_dir.display()));
                    ui.label(format!("Library: {}", config.paths.library_dir.display()));
                }
            });

        egui::CollapsingHeader::new("Logging")
            .default_open(matches!(self.active_pane, PrefPane::SystemLogging))
            .show(ui, |ui| {
                if self.edit_mode {
                    ui.horizontal(|ui| {
                        ui.label("Level");
                        egui::ComboBox::from_id_salt("logging_level")
                            .selected_text(self.state.logging_level.clone())
                            .show_ui(ui, |ui| {
                                for level in ["trace", "debug", "info", "warn", "error"] {
                                    ui.selectable_value(
                                        &mut self.state.logging_level,
                                        level.to_string(),
                                        level,
                                    );
                                }
                            });
                    });
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
            if self.edit_mode {
                ui.horizontal(|ui| {
                    ui.label("SQLite");
                    ui.text_edit_singleline(&mut self.state.db_sqlite_path);
                });
                ui.horizontal(|ui| {
                    ui.label("Pool size");
                    ui.add(egui::DragValue::new(&mut self.state.db_pool_size).range(1..=64));
                });
                ui.horizontal(|ui| {
                    ui.label("Busy timeout (ms)");
                    ui.add(
                        egui::DragValue::new(&mut self.state.db_busy_timeout_ms).range(100..=60000),
                    );
                });
            } else {
                ui.label(format!("SQLite: {}", config.db.sqlite_path.display()));
                ui.label(format!("Pool size: {}", config.db.pool_size));
                ui.label(format!("Busy timeout (ms): {}", config.db.busy_timeout_ms));
            }
        });

        egui::CollapsingHeader::new("Network")
            .default_open(matches!(self.active_pane, PrefPane::SystemNetwork))
            .show(ui, |ui| {
                if self.edit_mode {
                    ui.horizontal(|ui| {
                        ui.label("HTTP proxy");
                        ui.text_edit_singleline(&mut self.state.network_http_proxy);
                    });
                    ui.horizontal(|ui| {
                        ui.label("HTTPS proxy");
                        ui.text_edit_singleline(&mut self.state.network_https_proxy);
                    });
                    ui.horizontal(|ui| {
                        ui.label("No proxy");
                        ui.text_edit_singleline(&mut self.state.network_no_proxy);
                    });
                    ui.checkbox(&mut self.state.network_offline_mode, "Offline mode");
                    ui.horizontal(|ui| {
                        ui.label("DNS mode");
                        egui::ComboBox::from_id_salt("network_dns_mode")
                            .selected_text(self.state.network_dns_mode.clone())
                            .show_ui(ui, |ui| {
                                for mode in ["system", "doh_cloudflare", "doh_google"] {
                                    ui.selectable_value(
                                        &mut self.state.network_dns_mode,
                                        mode.to_string(),
                                        mode,
                                    );
                                }
                            });
                    });
                } else {
                    ui.label(format!("HTTP proxy: {}", config.network.http_proxy));
                    ui.label(format!("HTTPS proxy: {}", config.network.https_proxy));
                    ui.label(format!("No proxy: {}", config.network.no_proxy));
                    ui.label(format!("Offline mode: {}", config.network.offline_mode));
                    ui.label(format!("DNS mode: {}", config.network.dns_mode));
                }
            });
    }

    fn render_behavior_section(&mut self, ui: &mut egui::Ui, config: &ControlPlane) {
        egui::CollapsingHeader::new("Assets")
            .default_open(matches!(self.active_pane, PrefPane::BehaviorAssets))
            .show(ui, |ui| {
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

        egui::CollapsingHeader::new("Ingest")
            .default_open(matches!(self.active_pane, PrefPane::BehaviorIngest))
            .show(ui, |ui| {
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

        egui::CollapsingHeader::new("Library")
            .default_open(matches!(self.active_pane, PrefPane::BehaviorLibrary))
            .show(ui, |ui| {
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
        egui::CollapsingHeader::new("GUI")
            .default_open(matches!(self.active_pane, PrefPane::LookGui))
            .show(ui, |ui| {
                if self.edit_mode {
                    ui.horizontal(|ui| {
                        ui.label("App theme");
                        egui::ComboBox::from_id_salt("gui_app_theme")
                            .selected_text(self.state.gui_app_theme.clone())
                            .show_ui(ui, |ui| {
                                for value in ["system", "light", "dark"] {
                                    ui.selectable_value(
                                        &mut self.state.gui_app_theme,
                                        value.to_string(),
                                        value,
                                    );
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Icon set");
                        egui::ComboBox::from_id_salt("gui_icon_set")
                            .selected_text(self.state.gui_icon_set.clone())
                            .show_ui(ui, |ui| {
                                for value in ["calibre", "outline", "minimal"] {
                                    ui.selectable_value(
                                        &mut self.state.gui_icon_set,
                                        value.to_string(),
                                        value,
                                    );
                                }
                            });
                    });
                    ui.checkbox(
                        &mut self.state.gui_startup_open_last_library,
                        "Open last library on startup",
                    );
                    ui.checkbox(
                        &mut self.state.gui_startup_restore_tabs,
                        "Restore tabs on startup",
                    );
                    ui.horizontal(|ui| {
                        ui.label("System tray");
                        egui::ComboBox::from_id_salt("gui_system_tray_mode")
                            .selected_text(self.state.gui_system_tray_mode.clone())
                            .show_ui(ui, |ui| {
                                for value in ["disabled", "minimize_to_tray", "close_to_tray"] {
                                    ui.selectable_value(
                                        &mut self.state.gui_system_tray_mode,
                                        value.to_string(),
                                        value,
                                    );
                                }
                            });
                    });
                    ui.checkbox(
                        &mut self.state.gui_confirm_exit_with_jobs,
                        "Confirm exit when jobs are running",
                    );
                } else {
                    ui.label(format!("List view mode: {}", config.gui.list_view_mode));
                    ui.label(format!("Row height: {}", config.gui.table_row_height));
                    ui.label(format!("App theme: {}", config.gui.app_theme));
                    ui.label(format!("Icon set: {}", config.gui.icon_set));
                    ui.label(format!(
                        "Open last library on startup: {}",
                        config.gui.startup_open_last_library
                    ));
                    ui.label(format!(
                        "Restore tabs on startup: {}",
                        config.gui.startup_restore_tabs
                    ));
                    ui.label(format!("System tray mode: {}", config.gui.system_tray_mode));
                    ui.label(format!(
                        "Confirm exit with jobs: {}",
                        config.gui.confirm_exit_with_jobs
                    ));
                }
                let preview = match self.state.gui_app_theme.as_str() {
                    "dark" => egui::Color32::from_rgb(60, 60, 60),
                    "light" => egui::Color32::from_rgb(230, 230, 230),
                    _ => egui::Color32::from_rgb(130, 130, 130),
                };
                ui.colored_label(preview, "Theme preview swatch");
            });
        egui::CollapsingHeader::new("Reader")
            .default_open(matches!(self.active_pane, PrefPane::LookReader))
            .show(ui, |ui| {
                if self.edit_mode {
                    ui.horizontal(|ui| {
                        ui.label("Font size");
                        ui.add(
                            egui::DragValue::new(&mut self.state.reader_font_size)
                                .range(10.0..=28.0),
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
                            egui::DragValue::new(&mut self.state.reader_page_chars)
                                .range(600..=6000),
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

    fn render_import_export_section(&mut self, ui: &mut egui::Ui, config: &mut ControlPlane) {
        egui::CollapsingHeader::new("Preferences Import/Export")
            .default_open(matches!(self.active_pane, PrefPane::ImportExportPrefs))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Export path");
                    ui.text_edit_singleline(&mut self.export_path_input);
                    if ui.button("Export").clicked() {
                        let path = std::path::PathBuf::from(self.export_path_input.trim());
                        match self.export_preferences(config, &path) {
                            Ok(()) => {
                                self.status = format!("Preferences exported to {}", path.display())
                            }
                            Err(err) => self.last_error = Some(err.to_string()),
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Import path");
                    ui.text_edit_singleline(&mut self.import_path_input);
                    if ui
                        .add_enabled(self.edit_mode, egui::Button::new("Import"))
                        .clicked()
                    {
                        let path = std::path::PathBuf::from(self.import_path_input.trim());
                        match self.import_preferences(config, &path) {
                            Ok(()) => {
                                self.state = PreferencesState::from_config(config);
                                self.status =
                                    format!("Preferences imported from {}", path.display());
                            }
                            Err(err) => self.last_error = Some(err.to_string()),
                        }
                    }
                });
            });

        egui::CollapsingHeader::new("Conversion")
            .default_open(matches!(self.active_pane, PrefPane::ImportExportConversion))
            .show(ui, |ui| {
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

        egui::CollapsingHeader::new("Formats")
            .default_open(matches!(self.active_pane, PrefPane::ImportExportFormats))
            .show(ui, |ui| {
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
        egui::CollapsingHeader::new("Server")
            .default_open(matches!(self.active_pane, PrefPane::AdvancedServer))
            .show(ui, |ui| {
                if self.edit_mode {
                    ui.text_edit_singleline(&mut self.state.server_host);
                    if self.state.server_host.trim().is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(190, 0, 0),
                            "Host cannot be empty",
                        );
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
                    ui.checkbox(&mut self.state.server_tls_enabled, "TLS enabled");
                    ui.horizontal(|ui| {
                        ui.label("TLS cert path");
                        ui.text_edit_singleline(&mut self.state.server_tls_cert_path);
                    });
                    ui.horizontal(|ui| {
                        ui.label("TLS key path");
                        ui.text_edit_singleline(&mut self.state.server_tls_key_path);
                    });
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
                    ui.label(format!("TLS enabled: {}", config.server.tls_enabled));
                    ui.label(format!(
                        "TLS cert path: {}",
                        config.server.tls_cert_path.display()
                    ));
                    ui.label(format!(
                        "TLS key path: {}",
                        config.server.tls_key_path.display()
                    ));
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

        egui::CollapsingHeader::new("FTS")
            .default_open(matches!(self.active_pane, PrefPane::AdvancedFts))
            .show(ui, |ui| {
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
                        ui.add(
                            egui::DragValue::new(&mut self.state.fts_min_query_len).range(1..=20),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Result limit");
                        ui.add(
                            egui::DragValue::new(&mut self.state.fts_result_limit).range(1..=500),
                        );
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

        egui::CollapsingHeader::new("Metrics")
            .default_open(matches!(self.active_pane, PrefPane::AdvancedMetrics))
            .show(ui, |ui| {
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
        if self.state.server_tls_enabled
            && (self.state.server_tls_cert_path.trim().is_empty()
                || self.state.server_tls_key_path.trim().is_empty())
        {
            return Err(CoreError::ConfigValidate(
                "tls cert and key are required when TLS is enabled".to_string(),
            ));
        }
        if self.state.db_sqlite_path.trim().is_empty() {
            return Err(CoreError::ConfigValidate(
                "db sqlite path cannot be empty".to_string(),
            ));
        }
        if self.state.path_data_dir.trim().is_empty()
            || self.state.path_cache_dir.trim().is_empty()
            || self.state.path_log_dir.trim().is_empty()
            || self.state.path_tmp_dir.trim().is_empty()
            || self.state.path_library_dir.trim().is_empty()
        {
            return Err(CoreError::ConfigValidate(
                "path values cannot be empty".to_string(),
            ));
        }

        config.logging.level = self.state.logging_level.trim().to_string();
        config.logging.json = self.state.logging_json;
        config.logging.stdout = self.state.logging_stdout;
        config.logging.file_enabled = self.state.logging_file_enabled;
        config.db.sqlite_path = self.state.db_sqlite_path.trim().into();
        config.db.pool_size = self.state.db_pool_size;
        config.db.busy_timeout_ms = self.state.db_busy_timeout_ms;
        config.paths.data_dir = self.state.path_data_dir.trim().into();
        config.paths.cache_dir = self.state.path_cache_dir.trim().into();
        config.paths.log_dir = self.state.path_log_dir.trim().into();
        config.paths.tmp_dir = self.state.path_tmp_dir.trim().into();
        config.paths.library_dir = self.state.path_library_dir.trim().into();
        config.network.http_proxy = self.state.network_http_proxy.trim().to_string();
        config.network.https_proxy = self.state.network_https_proxy.trim().to_string();
        config.network.no_proxy = self.state.network_no_proxy.trim().to_string();
        config.network.offline_mode = self.state.network_offline_mode;
        config.network.dns_mode = self.state.network_dns_mode.trim().to_string();
        config.server.host = self.state.server_host.trim().to_string();
        config.server.port = self.state.server_port;
        config.server.scheme = self.state.server_scheme.trim().to_string();
        config.server.url_prefix = self.state.server_url_prefix.trim().to_string();
        config.server.enable_auth = self.state.server_enable_auth;
        config.server.tls_enabled = self.state.server_tls_enabled;
        config.server.tls_cert_path = self.state.server_tls_cert_path.trim().into();
        config.server.tls_key_path = self.state.server_tls_key_path.trim().into();
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
        config.gui.app_theme = self.state.gui_app_theme.trim().to_string();
        config.gui.icon_set = self.state.gui_icon_set.trim().to_string();
        config.gui.startup_open_last_library = self.state.gui_startup_open_last_library;
        config.gui.startup_restore_tabs = self.state.gui_startup_restore_tabs;
        config.gui.system_tray_mode = self.state.gui_system_tray_mode.trim().to_string();
        config.gui.confirm_exit_with_jobs = self.state.gui_confirm_exit_with_jobs;

        Ok(())
    }

    fn export_preferences(&self, config: &ControlPlane, path: &std::path::Path) -> CoreResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                CoreError::Io("create preferences export parent".to_string(), err)
            })?;
        }
        config.save_to_path(path)?;
        info!(component = "gui_preferences", path = %path.display(), "exported preferences file");
        Ok(())
    }

    fn import_preferences(
        &self,
        config: &mut ControlPlane,
        path: &std::path::Path,
    ) -> CoreResult<()> {
        let imported = ControlPlane::load_from_path(path)?;
        *config = imported;
        info!(component = "gui_preferences", path = %path.display(), "imported preferences file");
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
        if !self.state.server_url_prefix.trim().is_empty()
            && !self.state.server_url_prefix.starts_with('/')
        {
            errors.push("server url prefix must start with '/'".to_string());
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
        if self.state.db_sqlite_path.trim().is_empty() {
            errors.push("db sqlite path must not be empty".to_string());
        }
        if !matches!(self.state.reader_theme.as_str(), "light" | "dark" | "sepia") {
            errors.push("reader theme must be light, dark, or sepia".to_string());
        }
        if !matches!(
            self.state.gui_app_theme.as_str(),
            "system" | "light" | "dark"
        ) {
            errors.push("app theme must be system, light, or dark".to_string());
        }
        if !matches!(
            self.state.gui_icon_set.as_str(),
            "calibre" | "outline" | "minimal"
        ) {
            errors.push("icon set must be calibre, outline, or minimal".to_string());
        }
        if !matches!(
            self.state.gui_system_tray_mode.as_str(),
            "disabled" | "minimize_to_tray" | "close_to_tray"
        ) {
            errors.push(
                "system tray mode must be disabled, minimize_to_tray, or close_to_tray".to_string(),
            );
        }
        if !matches!(
            self.state.network_dns_mode.as_str(),
            "system" | "doh_cloudflare" | "doh_google"
        ) {
            errors
                .push("network dns mode must be system, doh_cloudflare, or doh_google".to_string());
        }
        if self.state.server_tls_enabled
            && (self.state.server_tls_cert_path.trim().is_empty()
                || self.state.server_tls_key_path.trim().is_empty())
        {
            errors.push("tls cert and key must be set when TLS is enabled".to_string());
        }
        errors
    }

    fn reset_active_section_to_defaults(&mut self) {
        let defaults = PreferencesState::defaults();
        match self.active_section {
            PrefSection::Behavior => {
                self.state.assets_compress_raw = defaults.assets_compress_raw;
                self.state.assets_compress_metadata = defaults.assets_compress_metadata;
                self.state.ingest_default_mode = defaults.ingest_default_mode;
                self.state.ingest_archive_reference_enabled =
                    defaults.ingest_archive_reference_enabled;
                self.state.ingest_duplicate_policy = defaults.ingest_duplicate_policy;
                self.state.ingest_duplicate_compare = defaults.ingest_duplicate_compare;
            }
            PrefSection::LookAndFeel => {
                self.state.reader_font_size = defaults.reader_font_size;
                self.state.reader_line_spacing = defaults.reader_line_spacing;
                self.state.reader_page_chars = defaults.reader_page_chars;
                self.state.reader_theme = defaults.reader_theme;
                self.state.gui_app_theme = defaults.gui_app_theme;
                self.state.gui_icon_set = defaults.gui_icon_set;
                self.state.gui_startup_open_last_library = defaults.gui_startup_open_last_library;
                self.state.gui_startup_restore_tabs = defaults.gui_startup_restore_tabs;
                self.state.gui_system_tray_mode = defaults.gui_system_tray_mode;
                self.state.gui_confirm_exit_with_jobs = defaults.gui_confirm_exit_with_jobs;
            }
            PrefSection::ImportExport => {
                self.state.conversion_enabled = defaults.conversion_enabled;
                self.state.conversion_allow_passthrough = defaults.conversion_allow_passthrough;
                self.state.conversion_max_input_bytes = defaults.conversion_max_input_bytes;
                self.state.conversion_default_output_format =
                    defaults.conversion_default_output_format;
            }
            PrefSection::Advanced => {
                self.state.server_host = defaults.server_host;
                self.state.server_port = defaults.server_port;
                self.state.server_scheme = defaults.server_scheme;
                self.state.server_url_prefix = defaults.server_url_prefix;
                self.state.server_enable_auth = defaults.server_enable_auth;
                self.state.server_tls_enabled = defaults.server_tls_enabled;
                self.state.server_tls_cert_path = defaults.server_tls_cert_path;
                self.state.server_tls_key_path = defaults.server_tls_key_path;
                self.state.server_download_enabled = defaults.server_download_enabled;
                self.state.server_download_max_bytes = defaults.server_download_max_bytes;
                self.state.server_download_allow_external = defaults.server_download_allow_external;
                self.state.fts_enabled = defaults.fts_enabled;
                self.state.fts_tokenizer = defaults.fts_tokenizer;
                self.state.fts_rebuild_on_migrate = defaults.fts_rebuild_on_migrate;
                self.state.fts_min_query_len = defaults.fts_min_query_len;
                self.state.fts_result_limit = defaults.fts_result_limit;
            }
            PrefSection::System => {
                self.state.logging_level = defaults.logging_level;
                self.state.logging_json = defaults.logging_json;
                self.state.logging_stdout = defaults.logging_stdout;
                self.state.logging_file_enabled = defaults.logging_file_enabled;
                self.state.db_sqlite_path = defaults.db_sqlite_path;
                self.state.db_pool_size = defaults.db_pool_size;
                self.state.db_busy_timeout_ms = defaults.db_busy_timeout_ms;
                self.state.path_data_dir = defaults.path_data_dir;
                self.state.path_cache_dir = defaults.path_cache_dir;
                self.state.path_log_dir = defaults.path_log_dir;
                self.state.path_tmp_dir = defaults.path_tmp_dir;
                self.state.path_library_dir = defaults.path_library_dir;
                self.state.network_http_proxy = defaults.network_http_proxy;
                self.state.network_https_proxy = defaults.network_https_proxy;
                self.state.network_no_proxy = defaults.network_no_proxy;
                self.state.network_offline_mode = defaults.network_offline_mode;
                self.state.network_dns_mode = defaults.network_dns_mode;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct PreferencesState {
    logging_level: String,
    logging_json: bool,
    logging_stdout: bool,
    logging_file_enabled: bool,
    db_sqlite_path: String,
    db_pool_size: u32,
    db_busy_timeout_ms: u64,
    path_data_dir: String,
    path_cache_dir: String,
    path_log_dir: String,
    path_tmp_dir: String,
    path_library_dir: String,
    network_http_proxy: String,
    network_https_proxy: String,
    network_no_proxy: String,
    network_offline_mode: bool,
    network_dns_mode: String,
    server_host: String,
    server_port: u16,
    server_scheme: String,
    server_url_prefix: String,
    server_enable_auth: bool,
    server_tls_enabled: bool,
    server_tls_cert_path: String,
    server_tls_key_path: String,
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
    gui_app_theme: String,
    gui_icon_set: String,
    gui_startup_open_last_library: bool,
    gui_startup_restore_tabs: bool,
    gui_system_tray_mode: String,
    gui_confirm_exit_with_jobs: bool,
}

impl PreferencesState {
    fn from_config(config: &ControlPlane) -> Self {
        Self {
            logging_level: config.logging.level.clone(),
            logging_json: config.logging.json,
            logging_stdout: config.logging.stdout,
            logging_file_enabled: config.logging.file_enabled,
            db_sqlite_path: config.db.sqlite_path.display().to_string(),
            db_pool_size: config.db.pool_size,
            db_busy_timeout_ms: config.db.busy_timeout_ms,
            path_data_dir: config.paths.data_dir.display().to_string(),
            path_cache_dir: config.paths.cache_dir.display().to_string(),
            path_log_dir: config.paths.log_dir.display().to_string(),
            path_tmp_dir: config.paths.tmp_dir.display().to_string(),
            path_library_dir: config.paths.library_dir.display().to_string(),
            network_http_proxy: config.network.http_proxy.clone(),
            network_https_proxy: config.network.https_proxy.clone(),
            network_no_proxy: config.network.no_proxy.clone(),
            network_offline_mode: config.network.offline_mode,
            network_dns_mode: config.network.dns_mode.clone(),
            server_host: config.server.host.clone(),
            server_port: config.server.port,
            server_scheme: config.server.scheme.clone(),
            server_url_prefix: config.server.url_prefix.clone(),
            server_enable_auth: config.server.enable_auth,
            server_tls_enabled: config.server.tls_enabled,
            server_tls_cert_path: config.server.tls_cert_path.display().to_string(),
            server_tls_key_path: config.server.tls_key_path.display().to_string(),
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
            gui_app_theme: config.gui.app_theme.clone(),
            gui_icon_set: config.gui.icon_set.clone(),
            gui_startup_open_last_library: config.gui.startup_open_last_library,
            gui_startup_restore_tabs: config.gui.startup_restore_tabs,
            gui_system_tray_mode: config.gui.system_tray_mode.clone(),
            gui_confirm_exit_with_jobs: config.gui.confirm_exit_with_jobs,
        }
    }

    fn defaults() -> Self {
        Self {
            logging_level: "info".to_string(),
            logging_json: false,
            logging_stdout: true,
            logging_file_enabled: false,
            db_sqlite_path: "./.cache/caliberate/data/caliberate.db".to_string(),
            db_pool_size: 4,
            db_busy_timeout_ms: 5000,
            path_data_dir: "./.cache/caliberate/data".to_string(),
            path_cache_dir: "./.cache/caliberate/cache".to_string(),
            path_log_dir: "./.cache/caliberate/logs".to_string(),
            path_tmp_dir: "./.cache/caliberate/tmp".to_string(),
            path_library_dir: "./.cache/caliberate/library".to_string(),
            network_http_proxy: String::new(),
            network_https_proxy: String::new(),
            network_no_proxy: String::new(),
            network_offline_mode: false,
            network_dns_mode: "system".to_string(),
            server_host: "127.0.0.1".to_string(),
            server_port: 8080,
            server_scheme: "http".to_string(),
            server_url_prefix: String::new(),
            server_enable_auth: false,
            server_tls_enabled: false,
            server_tls_cert_path: String::new(),
            server_tls_key_path: String::new(),
            server_download_enabled: true,
            server_download_max_bytes: 104_857_600,
            server_download_allow_external: false,
            assets_compress_raw: true,
            assets_compress_metadata: false,
            ingest_default_mode: IngestMode::Copy,
            ingest_archive_reference_enabled: true,
            ingest_duplicate_policy: DuplicatePolicy::Error,
            ingest_duplicate_compare: DuplicateCompare::Checksum,
            conversion_enabled: true,
            conversion_allow_passthrough: true,
            conversion_max_input_bytes: 104_857_600,
            conversion_default_output_format: "epub".to_string(),
            fts_enabled: false,
            fts_tokenizer: "unicode61 remove_diacritics 2".to_string(),
            fts_rebuild_on_migrate: true,
            fts_min_query_len: 2,
            fts_result_limit: 100,
            reader_font_size: 16.0,
            reader_line_spacing: 1.4,
            reader_page_chars: 1800,
            reader_theme: "light".to_string(),
            gui_app_theme: "system".to_string(),
            gui_icon_set: "calibre".to_string(),
            gui_startup_open_last_library: true,
            gui_startup_restore_tabs: true,
            gui_system_tray_mode: "disabled".to_string(),
            gui_confirm_exit_with_jobs: true,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrefPane {
    BehaviorAssets,
    BehaviorIngest,
    BehaviorLibrary,
    LookGui,
    LookReader,
    ImportExportPrefs,
    ImportExportConversion,
    ImportExportFormats,
    AdvancedServer,
    AdvancedFts,
    AdvancedMetrics,
    SystemApp,
    SystemPaths,
    SystemLogging,
    SystemNetwork,
}

impl PrefPane {
    fn section(self) -> PrefSection {
        match self {
            Self::BehaviorAssets | Self::BehaviorIngest | Self::BehaviorLibrary => {
                PrefSection::Behavior
            }
            Self::LookGui | Self::LookReader => PrefSection::LookAndFeel,
            Self::ImportExportPrefs | Self::ImportExportConversion | Self::ImportExportFormats => {
                PrefSection::ImportExport
            }
            Self::AdvancedServer | Self::AdvancedFts | Self::AdvancedMetrics => {
                PrefSection::Advanced
            }
            Self::SystemApp | Self::SystemPaths | Self::SystemLogging | Self::SystemNetwork => {
                PrefSection::System
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::BehaviorAssets => "Assets",
            Self::BehaviorIngest => "Ingest",
            Self::BehaviorLibrary => "Library",
            Self::LookGui => "GUI",
            Self::LookReader => "Reader",
            Self::ImportExportPrefs => "Preferences I/O",
            Self::ImportExportConversion => "Conversion",
            Self::ImportExportFormats => "Formats",
            Self::AdvancedServer => "Server",
            Self::AdvancedFts => "FTS",
            Self::AdvancedMetrics => "Metrics",
            Self::SystemApp => "App",
            Self::SystemPaths => "Paths",
            Self::SystemLogging => "Logging",
            Self::SystemNetwork => "Network",
        }
    }

    fn for_section(section: PrefSection) -> &'static [PrefPane] {
        match section {
            PrefSection::Behavior => &[
                PrefPane::BehaviorAssets,
                PrefPane::BehaviorIngest,
                PrefPane::BehaviorLibrary,
            ],
            PrefSection::LookAndFeel => &[PrefPane::LookGui, PrefPane::LookReader],
            PrefSection::ImportExport => &[
                PrefPane::ImportExportPrefs,
                PrefPane::ImportExportConversion,
                PrefPane::ImportExportFormats,
            ],
            PrefSection::Advanced => &[
                PrefPane::AdvancedServer,
                PrefPane::AdvancedFts,
                PrefPane::AdvancedMetrics,
            ],
            PrefSection::System => &[
                PrefPane::SystemApp,
                PrefPane::SystemPaths,
                PrefPane::SystemLogging,
                PrefPane::SystemNetwork,
            ],
        }
    }
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
    use super::{PrefPane, PrefSection, PreferencesState};
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

    #[test]
    fn pref_pane_section_mapping_is_stable() {
        assert_eq!(PrefPane::BehaviorAssets.section(), PrefSection::Behavior);
        assert_eq!(PrefPane::LookReader.section(), PrefSection::LookAndFeel);
        assert_eq!(
            PrefPane::ImportExportFormats.section(),
            PrefSection::ImportExport
        );
        assert_eq!(PrefPane::AdvancedFts.section(), PrefSection::Advanced);
        assert_eq!(PrefPane::SystemNetwork.section(), PrefSection::System);
    }

    #[test]
    fn pref_panes_exist_for_all_sections() {
        for section in PrefSection::all() {
            assert!(!PrefPane::for_section(section).is_empty());
        }
    }
}
