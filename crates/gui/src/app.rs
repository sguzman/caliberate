//! Application lifecycle wiring.

use caliberate_core::error::CoreResult;
use eframe::egui;

use crate::preferences::PreferencesView;
use crate::views::LibraryView;

pub struct CaliberateApp {
    config: caliberate_core::config::ControlPlane,
    config_path: std::path::PathBuf,
    library: LibraryView,
    preferences: PreferencesView,
    active_view: AppView,
    pending_action: Option<AppAction>,
}

impl CaliberateApp {
    pub fn try_new(
        config: caliberate_core::config::ControlPlane,
        config_path: std::path::PathBuf,
    ) -> CoreResult<Self> {
        let library = LibraryView::new(&config)?;
        let preferences = PreferencesView::new(&config);
        Ok(Self {
            config,
            config_path,
            library,
            preferences,
            active_view: AppView::Library,
            pending_action: None,
        })
    }
}

impl eframe::App for CaliberateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ui);
        egui::Panel::top("top_nav").show_inside(ui, |ui| {
            self.menu_bar(ui);
            ui.separator();
            self.toolbar(ui);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.active_view == AppView::Library, "Library")
                    .clicked()
                {
                    self.active_view = AppView::Library;
                }
                if ui
                    .selectable_label(self.active_view == AppView::Preferences, "Preferences")
                    .clicked()
                {
                    self.active_view = AppView::Preferences;
                }
            });
        });

        match self.active_view {
            AppView::Library => {
                self.library.ui(ui, &mut self.config, &self.config_path);
            }
            AppView::Preferences => {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    if let Err(err) = self.preferences.ui(ui, &mut self.config, &self.config_path) {
                        self.preferences.set_error(err.to_string());
                    }
                });
            }
        }

        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            let (status, error) = match self.active_view {
                AppView::Library => self.library.status_line(),
                AppView::Preferences => self.preferences.status_line(),
            };
            if let Some(err) = error {
                ui.colored_label(egui::Color32::from_rgb(190, 0, 0), err);
            } else {
                ui.label(status);
            }
        });

        if let Some(action) = self.pending_action.take() {
            self.apply_action(action);
        }

        self.error_dialog(ui);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppView {
    Library,
    Preferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppAction {
    FocusSearch,
    RefreshLibrary,
    BeginEdit,
    SaveEdit,
    OpenPreferences,
    OpenLibrary,
    AddBooks,
    RemoveBooks,
    ConvertBooks,
    SaveToDisk,
    OpenLogs,
}

impl CaliberateApp {
    fn error_dialog(&mut self, ui: &mut egui::Ui) {
        let error = self
            .library
            .error_message()
            .or_else(|| self.preferences.error_message());
        let Some(error) = error.map(|value| value.to_string()) else {
            return;
        };
        let mut error_text = error.clone();
        let mut open = true;
        let mut close_requested = false;
        egui::Window::new("Error")
            .collapsible(false)
            .resizable(true)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label("An error occurred:");
                ui.separator();
                ui.text_edit_multiline(&mut error_text);
                ui.separator();
                if ui.button("Copy details").clicked() {
                    ui.ctx().copy_text(error.clone());
                }
                if ui.button("Dismiss").clicked() {
                    close_requested = true;
                }
            });
        if close_requested {
            open = false;
        }
        if !open {
            self.library.clear_error_message();
            self.preferences.clear_error_message();
        }
    }
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Add books").clicked() {
                    self.pending_action = Some(AppAction::AddBooks);
                    ui.close_menu();
                }
                if ui.button("Save to disk").clicked() {
                    self.pending_action = Some(AppAction::SaveToDisk);
                    ui.close_menu();
                }
                if ui.button("Preferences").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferences);
                    ui.close_menu();
                }
            });
            ui.menu_button("Library", |ui| {
                if ui.button("Refresh").clicked() {
                    self.pending_action = Some(AppAction::RefreshLibrary);
                    ui.close_menu();
                }
                if ui.button("Remove books").clicked() {
                    self.pending_action = Some(AppAction::RemoveBooks);
                    ui.close_menu();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Edit metadata").clicked() {
                    self.pending_action = Some(AppAction::BeginEdit);
                    ui.close_menu();
                }
                if ui.button("Save metadata").clicked() {
                    self.pending_action = Some(AppAction::SaveEdit);
                    ui.close_menu();
                }
            });
            ui.menu_button("Convert", |ui| {
                if ui.button("Convert books").clicked() {
                    self.pending_action = Some(AppAction::ConvertBooks);
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Library").clicked() {
                    self.pending_action = Some(AppAction::OpenLibrary);
                    ui.close_menu();
                }
                if ui.button("Preferences").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferences);
                    ui.close_menu();
                }
                if ui.button("Open logs").clicked() {
                    self.pending_action = Some(AppAction::OpenLogs);
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label("Caliberate GUI");
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Add").clicked() {
                self.pending_action = Some(AppAction::AddBooks);
            }
            if ui.button("Remove").clicked() {
                self.pending_action = Some(AppAction::RemoveBooks);
            }
            if ui.button("Convert").clicked() {
                self.pending_action = Some(AppAction::ConvertBooks);
            }
            if ui.button("Save to Disk").clicked() {
                self.pending_action = Some(AppAction::SaveToDisk);
            }
            if ui.button("Refresh").clicked() {
                self.pending_action = Some(AppAction::RefreshLibrary);
            }
            if ui.button("Preferences").clicked() {
                self.pending_action = Some(AppAction::OpenPreferences);
            }
            if ui.button("Open Logs").clicked() {
                self.pending_action = Some(AppAction::OpenLogs);
            }
        });
    }

    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        if ui.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.ctrl) {
            self.pending_action = Some(AppAction::FocusSearch);
        }
        if ui.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
            self.pending_action = Some(AppAction::RefreshLibrary);
        }
        if ui.input(|i| i.key_pressed(egui::Key::E) && i.modifiers.ctrl) {
            self.pending_action = Some(AppAction::BeginEdit);
        }
        if ui.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.ctrl) {
            self.pending_action = Some(AppAction::SaveEdit);
        }
        if ui.input(|i| i.key_pressed(egui::Key::P) && i.modifiers.ctrl) {
            self.pending_action = Some(AppAction::OpenPreferences);
        }
        if ui.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
            self.pending_action = Some(AppAction::OpenLibrary);
        }
    }

    fn apply_action(&mut self, action: AppAction) {
        match action {
            AppAction::OpenPreferences => {
                self.active_view = AppView::Preferences;
            }
            AppAction::OpenLibrary => {
                self.active_view = AppView::Library;
            }
            AppAction::FocusSearch => {
                self.library.request_search_focus();
            }
            AppAction::RefreshLibrary => {
                self.library.request_refresh();
            }
            AppAction::BeginEdit => {
                self.library.begin_edit();
            }
            AppAction::SaveEdit => {
                self.library.request_save();
            }
            AppAction::AddBooks => {
                self.library.open_add_books(&self.config);
            }
            AppAction::RemoveBooks => {
                self.library.open_remove_books(&self.config);
            }
            AppAction::ConvertBooks => {
                self.library.open_convert_books(&self.config);
            }
            AppAction::SaveToDisk => {
                self.library.open_save_to_disk(&self.config);
            }
            AppAction::OpenLogs => {
                self.library.request_open_logs();
            }
        }
    }
}
