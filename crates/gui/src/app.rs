//! Application lifecycle wiring.

use caliberate_core::error::CoreResult;
use eframe::egui;

use crate::preferences::PreferencesView;
use crate::views::{LibraryView, PaneSide, ShellPaneLayout};
use tracing::{debug, info, warn};

pub struct CaliberateApp {
    config: caliberate_core::config::ControlPlane,
    config_path: std::path::PathBuf,
    library: LibraryView,
    preferences: PreferencesView,
    active_view: AppView,
    pending_action: Option<AppAction>,
    nav_back: Vec<AppView>,
    nav_forward: Vec<AppView>,
    command_palette_open: bool,
    command_palette_query: String,
    global_search_query: String,
    global_search_scope: GlobalSearchScope,
    global_search_count: usize,
    notification_center_open: bool,
    toolbar: ToolbarConfig,
    shortcuts: ShortcutBindings,
    shortcut_editor_open: bool,
    drag_drop_hints_enabled: bool,
    mouse_gestures_enabled: bool,
    gesture_accum_x: f32,
    shell_config_dirty: bool,
    recent_libraries: Vec<String>,
    recent_libraries_max: usize,
    active_library_label: String,
    library_switcher_open: bool,
    open_library_dialog_open: bool,
    open_library_input: String,
    create_library_dialog_open: bool,
    create_library_dir_input: String,
    pane_browser_visible: bool,
    pane_browser_side: PaneSide,
    pane_details_visible: bool,
    pane_details_side: PaneSide,
    pane_jobs_visible: bool,
    pane_left_width: f32,
    pane_right_width: f32,
    layout_preset: String,
}

impl CaliberateApp {
    fn apply_theme(&self, ui: &mut egui::Ui) {
        match self.config.gui.app_theme.as_str() {
            "dark" => ui.ctx().set_visuals(egui::Visuals::dark()),
            "light" => ui.ctx().set_visuals(egui::Visuals::light()),
            _ => {}
        }
    }

    pub fn try_new(
        mut config: caliberate_core::config::ControlPlane,
        config_path: std::path::PathBuf,
    ) -> CoreResult<Self> {
        if config.gui.startup_open_last_library {
            if let Some(path) = config.gui.recent_libraries.first() {
                if !path.trim().is_empty() {
                    let candidate = std::path::PathBuf::from(path);
                    if candidate.exists() {
                        config.db.sqlite_path = candidate;
                    }
                }
            }
        }
        let pane_layout = ShellPaneLayout {
            browser_visible: config.gui.pane_browser_visible,
            browser_side: parse_pane_side(&config.gui.pane_browser_side),
            details_visible: config.gui.pane_details_visible,
            details_side: parse_pane_side(&config.gui.pane_details_side),
            jobs_visible: config.gui.pane_jobs_visible,
            left_width: config.gui.pane_left_width,
            right_width: config.gui.pane_right_width,
        };
        let mut library = LibraryView::new(&config)?;
        library.set_shell_layout(pane_layout);
        let preferences = PreferencesView::new(&config);
        let toolbar = ToolbarConfig::from_actions(
            config.gui.toolbar_icon_only,
            &config.gui.toolbar_visible_actions,
        );
        let shortcuts = if config.gui.shortcut_preset == "calibre_like" {
            ShortcutBindings::calibre_like()
        } else {
            ShortcutBindings::default()
        };
        let drag_drop_hints_enabled = config.gui.drag_drop_hints;
        let global_search_scope = GlobalSearchScope::from_config(&config.gui.global_search_scope);
        let mut recent_libraries = config.gui.recent_libraries.clone();
        if recent_libraries.is_empty() {
            recent_libraries.push(config.db.sqlite_path.display().to_string());
        }
        let recent_libraries_max = config.gui.recent_libraries_max;
        let active_library_label = config.gui.active_library_label.clone();
        let mouse_gestures_enabled = config.gui.mouse_gestures;
        let pane_browser_visible = config.gui.pane_browser_visible;
        let pane_browser_side = parse_pane_side(&config.gui.pane_browser_side);
        let pane_details_visible = config.gui.pane_details_visible;
        let pane_details_side = parse_pane_side(&config.gui.pane_details_side);
        let pane_jobs_visible = config.gui.pane_jobs_visible;
        let pane_left_width = config.gui.pane_left_width;
        let pane_right_width = config.gui.pane_right_width;
        let layout_preset = config.gui.layout_preset.clone();
        let active_view =
            if config.gui.startup_restore_tabs && config.gui.last_active_view == "preferences" {
                AppView::Preferences
            } else {
                AppView::Library
            };
        Ok(Self {
            config,
            config_path,
            library,
            preferences,
            active_view,
            pending_action: None,
            nav_back: Vec::new(),
            nav_forward: Vec::new(),
            command_palette_open: false,
            command_palette_query: String::new(),
            global_search_query: String::new(),
            global_search_scope,
            global_search_count: 0,
            notification_center_open: false,
            toolbar,
            shortcuts,
            shortcut_editor_open: false,
            drag_drop_hints_enabled,
            mouse_gestures_enabled,
            gesture_accum_x: 0.0,
            shell_config_dirty: false,
            recent_libraries,
            recent_libraries_max,
            active_library_label,
            library_switcher_open: false,
            open_library_dialog_open: false,
            open_library_input: String::new(),
            create_library_dialog_open: false,
            create_library_dir_input: String::new(),
            pane_browser_visible,
            pane_browser_side,
            pane_details_visible,
            pane_details_side,
            pane_jobs_visible,
            pane_left_width,
            pane_right_width,
            layout_preset,
        })
    }
}

impl eframe::App for CaliberateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.apply_theme(ui);
        self.handle_shortcuts(ui);
        self.handle_mouse_gestures(ui);
        self.handle_dropped_files(ui);
        self.capture_window_geometry(ui);
        egui::Panel::top("top_nav").show_inside(ui, |ui| {
            self.menu_bar(ui);
            ui.separator();
            self.toolbar(ui);
            ui.separator();
            self.global_search_bar(ui);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(self.active_view == AppView::Library, "Library")
                    .clicked()
                {
                    self.switch_view(AppView::Library);
                }
                if ui
                    .selectable_label(self.active_view == AppView::Preferences, "Preferences")
                    .clicked()
                {
                    self.switch_view(AppView::Preferences);
                }
                let active_jobs = self.library.active_jobs_count();
                if active_jobs > 0 {
                    ui.colored_label(
                        egui::Color32::from_rgb(230, 170, 70),
                        format!("Busy: {active_jobs}"),
                    );
                }
                if ui.button("Notifications").clicked() {
                    self.notification_center_open = true;
                }
                ui.label(format!("Library: {}", self.active_library_label));
                ui.label(format!("Known libraries: {}", self.recent_libraries.len()));
            });
        });

        match self.active_view {
            AppView::Library => {
                self.sync_library_layout();
                self.library.ui(ui, &mut self.config, &self.config_path);
                self.pull_layout_from_library();
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
            ui.separator();
            ui.label(format!("DB: {}", self.config.db.sqlite_path.display()));
        });

        if let Some(action) = self.pending_action.take() {
            self.apply_action(action);
        }

        self.command_palette(ui);
        self.shortcut_editor(ui);
        self.notification_center(ui);
        self.library_switcher(ui);
        self.open_library_dialog(ui);
        self.create_library_dialog(ui);
        self.drag_drop_hint(ui);
        self.sync_shell_config();
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
    Back,
    Forward,
    ToggleNotifications,
    ToggleCommandPalette,
    ToggleShortcutEditor,
    OpenLibrarySwitcher,
    OpenLibraryDialog,
    CreateLibraryDialog,
    OpenDeviceSync,
    OpenManageTags,
    OpenManageSeries,
    OpenManageColumns,
    OpenManageVirtualLibraries,
    OpenFetchMetadata,
    OpenDownloadMetadata,
    OpenDownloadCover,
    OpenEditMetadataBulk,
    OpenViewBook,
    OpenRandomBook,
    OpenPreferencesInterface,
    OpenPreferencesBehavior,
    OpenPreferencesAdvanced,
    OpenPreferencesImportExport,
    OpenPreferencesSystem,
    OpenHelpAbout,
    OpenHelpUserManual,
    OpenHelpKeyboardShortcuts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlobalSearchScope {
    All,
    Title,
    Authors,
    Tags,
    Series,
}

impl GlobalSearchScope {
    fn from_config(value: &str) -> Self {
        match value {
            "title" => Self::Title,
            "authors" => Self::Authors,
            "tags" => Self::Tags,
            "series" => Self::Series,
            _ => Self::All,
        }
    }

    fn as_config(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Title => "title",
            Self::Authors => "authors",
            Self::Tags => "tags",
            Self::Series => "series",
        }
    }
}

fn parse_pane_side(value: &str) -> PaneSide {
    match value {
        "right" => PaneSide::Right,
        _ => PaneSide::Left,
    }
}

fn pane_side_to_config(side: PaneSide) -> &'static str {
    match side {
        PaneSide::Left => "left",
        PaneSide::Right => "right",
    }
}

#[derive(Debug, Clone, Copy)]
struct ToolbarConfig {
    show_add: bool,
    show_remove: bool,
    show_convert: bool,
    show_save_to_disk: bool,
    show_refresh: bool,
    show_preferences: bool,
    show_logs: bool,
    icon_only: bool,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            show_add: true,
            show_remove: true,
            show_convert: true,
            show_save_to_disk: true,
            show_refresh: true,
            show_preferences: true,
            show_logs: true,
            icon_only: false,
        }
    }
}

impl ToolbarConfig {
    fn from_actions(icon_only: bool, actions: &[String]) -> Self {
        let mut cfg = Self::default();
        cfg.icon_only = icon_only;
        cfg.show_add = actions.iter().any(|item| item == "add");
        cfg.show_remove = actions.iter().any(|item| item == "remove");
        cfg.show_convert = actions.iter().any(|item| item == "convert");
        cfg.show_save_to_disk = actions.iter().any(|item| item == "save_to_disk");
        cfg.show_refresh = actions.iter().any(|item| item == "refresh");
        cfg.show_preferences = actions.iter().any(|item| item == "preferences");
        cfg.show_logs = actions.iter().any(|item| item == "open_logs");
        cfg
    }

    fn to_actions(self) -> Vec<String> {
        let mut out = Vec::new();
        if self.show_add {
            out.push("add".to_string());
        }
        if self.show_remove {
            out.push("remove".to_string());
        }
        if self.show_convert {
            out.push("convert".to_string());
        }
        if self.show_save_to_disk {
            out.push("save_to_disk".to_string());
        }
        if self.show_refresh {
            out.push("refresh".to_string());
        }
        if self.show_preferences {
            out.push("preferences".to_string());
        }
        if self.show_logs {
            out.push("open_logs".to_string());
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShortcutBindings {
    focus_search: egui::KeyboardShortcut,
    refresh: egui::KeyboardShortcut,
    edit: egui::KeyboardShortcut,
    save: egui::KeyboardShortcut,
    preferences: egui::KeyboardShortcut,
    library: egui::KeyboardShortcut,
    command_palette: egui::KeyboardShortcut,
}

impl Default for ShortcutBindings {
    fn default() -> Self {
        Self {
            focus_search: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::F),
            refresh: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::R),
            edit: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::E),
            save: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S),
            preferences: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::P),
            library: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::L),
            command_palette: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::K),
        }
    }
}

impl ShortcutBindings {
    fn calibre_like() -> Self {
        Self {
            focus_search: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::F),
            refresh: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::F5),
            edit: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::E),
            save: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S),
            preferences: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::P),
            library: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::L),
            command_palette: egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::K),
        }
    }
}

impl CaliberateApp {
    fn switch_view(&mut self, next: AppView) {
        if self.active_view != next {
            self.nav_back.push(self.active_view);
            self.nav_forward.clear();
            self.active_view = next;
        }
    }

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

    fn apply_layout_preset(&mut self, preset: &str) {
        match preset {
            "focus" => {
                self.pane_browser_visible = false;
                self.pane_details_visible = true;
                self.pane_details_side = PaneSide::Right;
                self.pane_jobs_visible = true;
                self.pane_left_width = 760.0;
                self.pane_right_width = 520.0;
            }
            "minimal" => {
                self.pane_browser_visible = false;
                self.pane_details_visible = false;
                self.pane_jobs_visible = false;
                self.pane_left_width = 920.0;
                self.pane_right_width = 420.0;
            }
            "wide" => {
                self.pane_browser_visible = true;
                self.pane_browser_side = PaneSide::Left;
                self.pane_details_visible = true;
                self.pane_details_side = PaneSide::Right;
                self.pane_jobs_visible = true;
                self.pane_left_width = 680.0;
                self.pane_right_width = 620.0;
            }
            _ => {
                self.pane_browser_visible = true;
                self.pane_browser_side = PaneSide::Left;
                self.pane_details_visible = true;
                self.pane_details_side = PaneSide::Right;
                self.pane_jobs_visible = true;
                self.pane_left_width = 560.0;
                self.pane_right_width = 460.0;
            }
        }
        self.layout_preset = preset.to_string();
        self.shell_config_dirty = true;
        self.sync_library_layout();
    }

    fn sync_library_layout(&mut self) {
        self.library.set_shell_layout(ShellPaneLayout {
            browser_visible: self.pane_browser_visible,
            browser_side: self.pane_browser_side,
            details_visible: self.pane_details_visible,
            details_side: self.pane_details_side,
            jobs_visible: self.pane_jobs_visible,
            left_width: self.pane_left_width,
            right_width: self.pane_right_width,
        });
    }

    fn pull_layout_from_library(&mut self) {
        let layout = self.library.shell_layout();
        if (self.pane_left_width - layout.left_width).abs() > 0.5
            || (self.pane_right_width - layout.right_width).abs() > 0.5
        {
            self.shell_config_dirty = true;
        }
        self.pane_browser_visible = layout.browser_visible;
        self.pane_browser_side = layout.browser_side;
        self.pane_details_visible = layout.details_visible;
        self.pane_details_side = layout.details_side;
        self.pane_jobs_visible = layout.jobs_visible;
        self.pane_left_width = layout.left_width;
        self.pane_right_width = layout.right_width;
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
                if ui.button("View book").clicked() {
                    self.pending_action = Some(AppAction::OpenViewBook);
                    ui.close_menu();
                }
                if ui.button("Pick random book").clicked() {
                    self.pending_action = Some(AppAction::OpenRandomBook);
                    ui.close_menu();
                }
                if ui.button("Preferences").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferences);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Open library…").clicked() {
                    self.pending_action = Some(AppAction::OpenLibraryDialog);
                    ui.close_menu();
                }
                if ui.button("Create library…").clicked() {
                    self.pending_action = Some(AppAction::CreateLibraryDialog);
                    ui.close_menu();
                }
                if ui.button("Switch library…").clicked() {
                    self.pending_action = Some(AppAction::OpenLibrarySwitcher);
                    ui.close_menu();
                }
                ui.menu_button("Recent libraries", |ui| {
                    if self.recent_libraries.is_empty() {
                        ui.label("No recent libraries");
                    } else {
                        for path in self.recent_libraries.clone() {
                            if ui.button(path.clone()).clicked() {
                                if let Err(err) = self.switch_library(path.clone()) {
                                    self.preferences.set_error(err.to_string());
                                }
                                ui.close_menu();
                            }
                        }
                    }
                });
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
                if ui.button("Edit metadata in bulk").clicked() {
                    self.pending_action = Some(AppAction::OpenEditMetadataBulk);
                    ui.close_menu();
                }
            });
            ui.menu_button("Metadata", |ui| {
                if ui.button("Download metadata").clicked() {
                    self.pending_action = Some(AppAction::OpenDownloadMetadata);
                    ui.close_menu();
                }
                if ui.button("Fetch metadata by ID").clicked() {
                    self.pending_action = Some(AppAction::OpenFetchMetadata);
                    ui.close_menu();
                }
                if ui.button("Download cover").clicked() {
                    self.pending_action = Some(AppAction::OpenDownloadCover);
                    ui.close_menu();
                }
            });
            ui.menu_button("Convert", |ui| {
                if ui.button("Convert books").clicked() {
                    self.pending_action = Some(AppAction::ConvertBooks);
                    ui.close_menu();
                }
            });
            ui.menu_button("Navigate", |ui| {
                if ui.button("Back").clicked() {
                    self.pending_action = Some(AppAction::Back);
                    ui.close_menu();
                }
                if ui.button("Forward").clicked() {
                    self.pending_action = Some(AppAction::Forward);
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
                if ui.button("Command palette").clicked() {
                    self.pending_action = Some(AppAction::ToggleCommandPalette);
                    ui.close_menu();
                }
                if ui.button("Notifications").clicked() {
                    self.pending_action = Some(AppAction::ToggleNotifications);
                    ui.close_menu();
                }
                if ui.button("Shortcut editor").clicked() {
                    self.pending_action = Some(AppAction::ToggleShortcutEditor);
                    ui.close_menu();
                }
                ui.separator();
                ui.label("Toolbar");
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_add, "Show Add")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_remove, "Show Remove")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_convert, "Show Convert")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_save_to_disk, "Show Save to Disk")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_refresh, "Show Refresh")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_preferences, "Show Preferences")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.show_logs, "Show Open Logs")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.toolbar.icon_only, "Icon-only toolbar")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.drag_drop_hints_enabled, "Drag-drop hints")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.mouse_gestures_enabled, "Mouse gestures")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(
                        &mut self.config.gui.window_restore,
                        "Restore window on launch",
                    )
                    .changed();
                ui.separator();
                ui.label("Docked Panes");
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.pane_browser_visible, "Browser pane")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.pane_details_visible, "Details pane")
                    .changed();
                self.shell_config_dirty |= ui
                    .checkbox(&mut self.pane_jobs_visible, "Jobs pane")
                    .changed();
                let before_browser_side = self.pane_browser_side;
                egui::ComboBox::from_id_salt("pane_browser_side")
                    .selected_text(match self.pane_browser_side {
                        PaneSide::Left => "Browser Left",
                        PaneSide::Right => "Browser Right",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.pane_browser_side,
                            PaneSide::Left,
                            "Browser Left",
                        );
                        ui.selectable_value(
                            &mut self.pane_browser_side,
                            PaneSide::Right,
                            "Browser Right",
                        );
                    });
                if before_browser_side != self.pane_browser_side {
                    self.shell_config_dirty = true;
                }
                let before_details_side = self.pane_details_side;
                egui::ComboBox::from_id_salt("pane_details_side")
                    .selected_text(match self.pane_details_side {
                        PaneSide::Left => "Details Left",
                        PaneSide::Right => "Details Right",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.pane_details_side,
                            PaneSide::Left,
                            "Details Left",
                        );
                        ui.selectable_value(
                            &mut self.pane_details_side,
                            PaneSide::Right,
                            "Details Right",
                        );
                    });
                if before_details_side != self.pane_details_side {
                    self.shell_config_dirty = true;
                }
                if ui.button("Apply pane layout").clicked() {
                    self.sync_library_layout();
                    self.shell_config_dirty = true;
                }
                ui.separator();
                ui.label("Layout Presets");
                if ui.button("Classic").clicked() {
                    self.apply_layout_preset("classic");
                }
                if ui.button("Focus").clicked() {
                    self.apply_layout_preset("focus");
                }
                if ui.button("Minimal").clicked() {
                    self.apply_layout_preset("minimal");
                }
                if ui.button("Wide").clicked() {
                    self.apply_layout_preset("wide");
                }
            });
            ui.menu_button("Device", |ui| {
                if ui.button("Send to device").clicked() {
                    self.pending_action = Some(AppAction::OpenDeviceSync);
                    ui.close_menu();
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Manage tags").clicked() {
                    self.pending_action = Some(AppAction::OpenManageTags);
                    ui.close_menu();
                }
                if ui.button("Manage series").clicked() {
                    self.pending_action = Some(AppAction::OpenManageSeries);
                    ui.close_menu();
                }
                if ui.button("Manage custom columns").clicked() {
                    self.pending_action = Some(AppAction::OpenManageColumns);
                    ui.close_menu();
                }
                if ui.button("Virtual libraries").clicked() {
                    self.pending_action = Some(AppAction::OpenManageVirtualLibraries);
                    ui.close_menu();
                }
            });
            ui.menu_button("News", |ui| {
                ui.label("News panel roadmap in progress");
            });
            ui.menu_button("Preferences", |ui| {
                if ui.button("Interface").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferencesInterface);
                    ui.close_menu();
                }
                if ui.button("Behavior").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferencesBehavior);
                    ui.close_menu();
                }
                if ui.button("Advanced").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferencesAdvanced);
                    ui.close_menu();
                }
                if ui.button("Import/Export").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferencesImportExport);
                    ui.close_menu();
                }
                if ui.button("System").clicked() {
                    self.pending_action = Some(AppAction::OpenPreferencesSystem);
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.label("Caliberate GUI");
                ui.label("Ctrl+K for command palette");
                ui.separator();
                if ui.button("About").clicked() {
                    self.pending_action = Some(AppAction::OpenHelpAbout);
                    ui.close_menu();
                }
                if ui.button("User Manual").clicked() {
                    self.pending_action = Some(AppAction::OpenHelpUserManual);
                    ui.close_menu();
                }
                if ui.button("Keyboard Shortcuts").clicked() {
                    self.pending_action = Some(AppAction::OpenHelpKeyboardShortcuts);
                    ui.close_menu();
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let mut overflow = Vec::new();
            let compact = ui.available_width() < 760.0;
            if self.toolbar.show_add {
                self.toolbar_button(ui, compact, "＋", "Add", AppAction::AddBooks, &mut overflow);
            }
            if self.toolbar.show_remove {
                self.toolbar_button(
                    ui,
                    compact,
                    "－",
                    "Remove",
                    AppAction::RemoveBooks,
                    &mut overflow,
                );
            }
            if self.toolbar.show_convert {
                self.toolbar_button(
                    ui,
                    compact,
                    "⇄",
                    "Convert",
                    AppAction::ConvertBooks,
                    &mut overflow,
                );
            }
            if self.toolbar.show_save_to_disk {
                self.toolbar_button(
                    ui,
                    compact,
                    "⤓",
                    "Save",
                    AppAction::SaveToDisk,
                    &mut overflow,
                );
            }
            if self.toolbar.show_refresh {
                self.toolbar_button(
                    ui,
                    compact,
                    "↻",
                    "Refresh",
                    AppAction::RefreshLibrary,
                    &mut overflow,
                );
            }
            if self.toolbar.show_preferences {
                self.toolbar_button(
                    ui,
                    compact,
                    "⚙",
                    "Preferences",
                    AppAction::OpenPreferences,
                    &mut overflow,
                );
            }
            if self.toolbar.show_logs {
                self.toolbar_button(
                    ui,
                    compact,
                    "📝",
                    "Open Logs",
                    AppAction::OpenLogs,
                    &mut overflow,
                );
            }
            if !overflow.is_empty() {
                ui.menu_button("More", |ui| {
                    for (label, action) in overflow {
                        if ui.button(label).clicked() {
                            self.pending_action = Some(action);
                            ui.close_menu();
                        }
                    }
                });
            }
        });
    }

    fn handle_shortcuts(&mut self, ui: &mut egui::Ui) {
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.focus_search)) {
            self.pending_action = Some(AppAction::FocusSearch);
        }
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.refresh)) {
            self.pending_action = Some(AppAction::RefreshLibrary);
        }
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.edit)) {
            self.pending_action = Some(AppAction::BeginEdit);
        }
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.save)) {
            self.pending_action = Some(AppAction::SaveEdit);
        }
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.preferences)) {
            self.pending_action = Some(AppAction::OpenPreferences);
        }
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.library)) {
            self.pending_action = Some(AppAction::OpenLibrary);
        }
        if ui.input_mut(|i| i.consume_shortcut(&self.shortcuts.command_palette)) {
            self.pending_action = Some(AppAction::ToggleCommandPalette);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft) && i.modifiers.alt) {
            self.pending_action = Some(AppAction::Back);
        }
        if ui.input(|i| i.key_pressed(egui::Key::ArrowRight) && i.modifiers.alt) {
            self.pending_action = Some(AppAction::Forward);
        }
    }

    fn apply_action(&mut self, action: AppAction) {
        match action {
            AppAction::OpenPreferences => {
                self.switch_view(AppView::Preferences);
            }
            AppAction::OpenLibrary => {
                self.switch_view(AppView::Library);
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
            AppAction::Back => {
                if let Some(prev) = self.nav_back.pop() {
                    self.nav_forward.push(self.active_view);
                    self.active_view = prev;
                }
            }
            AppAction::Forward => {
                if let Some(next) = self.nav_forward.pop() {
                    self.nav_back.push(self.active_view);
                    self.active_view = next;
                }
            }
            AppAction::ToggleNotifications => {
                self.notification_center_open = !self.notification_center_open;
            }
            AppAction::ToggleCommandPalette => {
                self.command_palette_open = !self.command_palette_open;
                if self.command_palette_open {
                    self.command_palette_query.clear();
                }
            }
            AppAction::ToggleShortcutEditor => {
                self.shortcut_editor_open = !self.shortcut_editor_open;
            }
            AppAction::OpenLibrarySwitcher => {
                self.library_switcher_open = true;
            }
            AppAction::OpenLibraryDialog => {
                self.open_library_dialog_open = true;
                self.open_library_input = self.config.db.sqlite_path.display().to_string();
            }
            AppAction::CreateLibraryDialog => {
                self.create_library_dialog_open = true;
            }
            AppAction::OpenDeviceSync => {
                self.library.open_device_sync(&self.config);
            }
            AppAction::OpenManageTags => {
                self.library.open_manage_tags();
            }
            AppAction::OpenManageSeries => {
                self.library.open_manage_series();
            }
            AppAction::OpenManageColumns => {
                self.library.open_manage_custom_columns();
            }
            AppAction::OpenManageVirtualLibraries => {
                self.library.open_manage_virtual_libraries();
            }
            AppAction::OpenFetchMetadata => {
                self.library
                    .notify_unimplemented("Fetch metadata by ID is not implemented yet");
                info!(
                    component = "gui_shell",
                    "triggered fetch metadata placeholder action"
                );
            }
            AppAction::OpenDownloadMetadata => {
                self.library.open_download_metadata();
            }
            AppAction::OpenDownloadCover => {
                self.library.open_download_cover();
            }
            AppAction::OpenEditMetadataBulk => {
                self.library.open_bulk_edit();
            }
            AppAction::OpenViewBook => {
                self.library.notify_unimplemented(
                    "View book from shell menu is not implemented yet; use row context menu",
                );
            }
            AppAction::OpenRandomBook => {
                self.library.notify_unimplemented(
                    "Random book selection from shell is not implemented yet",
                );
            }
            AppAction::OpenPreferencesInterface => {
                self.switch_view(AppView::Preferences);
                self.preferences.open_section_look_and_feel();
            }
            AppAction::OpenPreferencesBehavior => {
                self.switch_view(AppView::Preferences);
                self.preferences.open_section_behavior();
            }
            AppAction::OpenPreferencesAdvanced => {
                self.switch_view(AppView::Preferences);
                self.preferences.open_section_advanced();
            }
            AppAction::OpenPreferencesImportExport => {
                self.switch_view(AppView::Preferences);
                self.preferences.open_section_import_export();
            }
            AppAction::OpenPreferencesSystem => {
                self.switch_view(AppView::Preferences);
                self.preferences.open_section_system();
            }
            AppAction::OpenHelpAbout => {
                self.library
                    .notify_unimplemented("About dialog is not implemented yet");
            }
            AppAction::OpenHelpUserManual => {
                self.library
                    .notify_unimplemented("User manual entry point is not implemented yet");
            }
            AppAction::OpenHelpKeyboardShortcuts => {
                self.shortcut_editor_open = true;
                self.notification_center_open = false;
            }
        }
    }

    fn toolbar_button(
        &mut self,
        ui: &mut egui::Ui,
        compact: bool,
        icon: &str,
        label: &str,
        action: AppAction,
        overflow: &mut Vec<(String, AppAction)>,
    ) {
        if compact {
            overflow.push((label.to_string(), action));
            return;
        }
        let text = if self.toolbar.icon_only {
            icon.to_string()
        } else {
            format!("{icon} {label}")
        };
        if ui.button(text).clicked() {
            self.pending_action = Some(action);
        }
    }

    fn global_search_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Global");
            let response = ui.text_edit_singleline(&mut self.global_search_query);
            let previous_scope = self.global_search_scope;
            egui::ComboBox::from_id_salt("global_scope")
                .selected_text(match self.global_search_scope {
                    GlobalSearchScope::All => "All",
                    GlobalSearchScope::Title => "Title",
                    GlobalSearchScope::Authors => "Authors",
                    GlobalSearchScope::Tags => "Tags",
                    GlobalSearchScope::Series => "Series",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.global_search_scope,
                        GlobalSearchScope::All,
                        "All",
                    );
                    ui.selectable_value(
                        &mut self.global_search_scope,
                        GlobalSearchScope::Title,
                        "Title",
                    );
                    ui.selectable_value(
                        &mut self.global_search_scope,
                        GlobalSearchScope::Authors,
                        "Authors",
                    );
                    ui.selectable_value(
                        &mut self.global_search_scope,
                        GlobalSearchScope::Tags,
                        "Tags",
                    );
                    ui.selectable_value(
                        &mut self.global_search_scope,
                        GlobalSearchScope::Series,
                        "Series",
                    );
                });
            if self.global_search_scope != previous_scope {
                self.shell_config_dirty = true;
            }
            if response.changed() || ui.button("Apply").clicked() {
                self.library.apply_global_search(
                    &self.global_search_query,
                    match self.global_search_scope {
                        GlobalSearchScope::All => "all",
                        GlobalSearchScope::Title => "title",
                        GlobalSearchScope::Authors => "authors",
                        GlobalSearchScope::Tags => "tags",
                        GlobalSearchScope::Series => "series",
                    },
                );
                self.global_search_count = self.library.filtered_count();
            }
            if ui.button("Clear").clicked() {
                self.global_search_query.clear();
                self.library.clear_search_query();
                self.global_search_count = self.library.filtered_count();
            }
            ui.label(format!("Results: {}", self.global_search_count));
        });
    }

    fn command_palette(&mut self, ui: &mut egui::Ui) {
        if !self.command_palette_open {
            return;
        }
        let mut open = self.command_palette_open;
        let mut selected = None::<AppAction>;
        egui::Window::new("Command Palette")
            .open(&mut open)
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                ui.text_edit_singleline(&mut self.command_palette_query);
                let query = self.command_palette_query.to_lowercase();
                let commands = vec![
                    ("Open Library", AppAction::OpenLibrary),
                    ("Open Preferences", AppAction::OpenPreferences),
                    ("Refresh Library", AppAction::RefreshLibrary),
                    ("Add Books", AppAction::AddBooks),
                    ("Download Metadata", AppAction::OpenDownloadMetadata),
                    ("Download Cover", AppAction::OpenDownloadCover),
                    ("Remove Books", AppAction::RemoveBooks),
                    ("Convert Books", AppAction::ConvertBooks),
                    ("Save To Disk", AppAction::SaveToDisk),
                    ("Open Logs", AppAction::OpenLogs),
                    ("Toggle Notifications", AppAction::ToggleNotifications),
                    ("Toggle Shortcut Editor", AppAction::ToggleShortcutEditor),
                ];
                for (label, action) in commands {
                    if !query.is_empty() && !label.to_lowercase().contains(&query) {
                        continue;
                    }
                    if ui.button(label).clicked() {
                        selected = Some(action);
                    }
                }
            });
        self.command_palette_open = open;
        if let Some(action) = selected {
            self.command_palette_open = false;
            self.pending_action = Some(action);
        }
    }

    fn notification_center(&mut self, ui: &mut egui::Ui) {
        if !self.notification_center_open {
            return;
        }
        let mut open = self.notification_center_open;
        egui::Window::new("Notification Center")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let notifications = self.library.recent_notifications(30);
                if notifications.is_empty() {
                    ui.label("No notifications.");
                } else {
                    for message in notifications {
                        ui.label(message);
                    }
                }
            });
        self.notification_center_open = open;
    }

    fn shortcut_editor(&mut self, ui: &mut egui::Ui) {
        if !self.shortcut_editor_open {
            return;
        }
        let mut open = self.shortcut_editor_open;
        egui::Window::new("Shortcut Editor")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Default preset").clicked() {
                        self.shortcuts = ShortcutBindings::default();
                        self.shell_config_dirty = true;
                    }
                    if ui.button("Calibre-like preset").clicked() {
                        self.shortcuts = ShortcutBindings::calibre_like();
                        self.shell_config_dirty = true;
                    }
                });
                ui.separator();
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Focus search", &mut self.shortcuts.focus_search);
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Refresh", &mut self.shortcuts.refresh);
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Edit metadata", &mut self.shortcuts.edit);
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Save metadata", &mut self.shortcuts.save);
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Open preferences", &mut self.shortcuts.preferences);
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Open library", &mut self.shortcuts.library);
                self.shell_config_dirty |=
                    Self::shortcut_row(ui, "Command palette", &mut self.shortcuts.command_palette);
                if self.shortcuts_have_conflict() {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        "Shortcut conflict detected",
                    );
                } else {
                    ui.label("No conflicts.");
                }
            });
        self.shortcut_editor_open = open;
    }

    fn shortcut_row(ui: &mut egui::Ui, label: &str, binding: &mut egui::KeyboardShortcut) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(label);
            let mut key = binding.logical_key;
            egui::ComboBox::from_id_salt(format!("shortcut_{label}"))
                .selected_text(format!("{:?}", key))
                .show_ui(ui, |ui| {
                    for candidate in [
                        egui::Key::A,
                        egui::Key::C,
                        egui::Key::E,
                        egui::Key::F,
                        egui::Key::K,
                        egui::Key::L,
                        egui::Key::P,
                        egui::Key::R,
                        egui::Key::S,
                        egui::Key::F5,
                    ] {
                        ui.selectable_value(&mut key, candidate, format!("{candidate:?}"));
                    }
                });
            if key != binding.logical_key {
                changed = true;
            }
            binding.logical_key = key;
            changed |= ui.checkbox(&mut binding.modifiers.ctrl, "Ctrl").changed();
            changed |= ui.checkbox(&mut binding.modifiers.shift, "Shift").changed();
            changed |= ui.checkbox(&mut binding.modifiers.alt, "Alt").changed();
        });
        changed
    }

    fn shortcuts_have_conflict(&self) -> bool {
        let all = [
            self.shortcuts.focus_search,
            self.shortcuts.refresh,
            self.shortcuts.edit,
            self.shortcuts.save,
            self.shortcuts.preferences,
            self.shortcuts.library,
            self.shortcuts.command_palette,
        ];
        for (idx, left) in all.iter().enumerate() {
            for right in all.iter().skip(idx + 1) {
                if left.modifiers == right.modifiers && left.logical_key == right.logical_key {
                    return true;
                }
            }
        }
        false
    }

    fn drag_drop_hint(&mut self, ui: &mut egui::Ui) {
        let hovering_files = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        if self.drag_drop_hints_enabled && hovering_files {
            egui::Area::new("drop_hint".into())
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::window(ui.style()).show(ui, |ui| {
                        ui.label("Drop files to add books");
                    });
                });
        }
    }

    fn handle_dropped_files(&mut self, ui: &mut egui::Ui) {
        let dropped = ui
            .ctx()
            .input(|i| i.raw.dropped_files.clone())
            .into_iter()
            .filter_map(|file| file.path)
            .collect::<Vec<_>>();
        if dropped.is_empty() {
            return;
        }
        info!(
            component = "gui_shell",
            dropped_count = dropped.len(),
            "received files via drag-and-drop ingest"
        );
        if let Err(err) = self.library.ingest_paths_now(&self.config, &dropped) {
            warn!(
                component = "gui_shell",
                error = %err,
                "drag-and-drop ingest failed"
            );
            self.preferences.set_error(err.to_string());
        }
    }

    fn handle_mouse_gestures(&mut self, ui: &mut egui::Ui) {
        if !self.mouse_gestures_enabled {
            return;
        }
        ui.ctx().input(|i| {
            if i.pointer.button_down(egui::PointerButton::Secondary) {
                self.gesture_accum_x += i.pointer.delta().x;
            } else if self.gesture_accum_x.abs() > 80.0 {
                if self.gesture_accum_x > 0.0 {
                    self.pending_action = Some(AppAction::Forward);
                    debug!(component = "gui_shell", "mouse gesture navigation forward");
                } else {
                    self.pending_action = Some(AppAction::Back);
                    debug!(component = "gui_shell", "mouse gesture navigation back");
                }
                self.gesture_accum_x = 0.0;
            } else {
                self.gesture_accum_x = 0.0;
            }
        });
    }

    fn capture_window_geometry(&mut self, ui: &mut egui::Ui) {
        let outer = ui.ctx().input(|i| i.viewport().outer_rect);
        if let Some(rect) = outer {
            let size = rect.size();
            let pos = rect.left_top();
            if (self.config.gui.window_width - size.x).abs() > 0.5
                || (self.config.gui.window_height - size.y).abs() > 0.5
                || (self.config.gui.window_pos_x - pos.x).abs() > 0.5
                || (self.config.gui.window_pos_y - pos.y).abs() > 0.5
            {
                self.config.gui.window_width = size.x.max(640.0);
                self.config.gui.window_height = size.y.max(480.0);
                self.config.gui.window_pos_x = pos.x;
                self.config.gui.window_pos_y = pos.y;
                self.shell_config_dirty = true;
            }
        }
    }

    fn library_switcher(&mut self, ui: &mut egui::Ui) {
        if !self.library_switcher_open {
            return;
        }
        let mut open = self.library_switcher_open;
        egui::Window::new("Library Switcher")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                if self.recent_libraries.is_empty() {
                    ui.label("No recent libraries.");
                } else {
                    for path in self.recent_libraries.clone() {
                        if ui.button(path.clone()).clicked() {
                            if let Err(err) = self.switch_library(path) {
                                self.preferences.set_error(err.to_string());
                            }
                        }
                    }
                }
            });
        self.library_switcher_open = open;
    }

    fn open_library_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.open_library_dialog_open {
            return;
        }
        let mut open = self.open_library_dialog_open;
        let mut confirmed = false;
        egui::Window::new("Open Library")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label("SQLite path");
                ui.text_edit_singleline(&mut self.open_library_input);
                if ui.button("Open").clicked() {
                    confirmed = true;
                }
            });
        if confirmed {
            if let Err(err) = self.switch_library(self.open_library_input.trim().to_string()) {
                self.preferences.set_error(err.to_string());
            } else {
                open = false;
            }
        }
        self.open_library_dialog_open = open;
    }

    fn create_library_dialog(&mut self, ui: &mut egui::Ui) {
        if !self.create_library_dialog_open {
            return;
        }
        let mut open = self.create_library_dialog_open;
        let mut confirmed = false;
        egui::Window::new("Create Library")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label("Library directory");
                ui.text_edit_singleline(&mut self.create_library_dir_input);
                if ui.button("Create").clicked() {
                    confirmed = true;
                }
            });
        if confirmed {
            let dir = std::path::PathBuf::from(self.create_library_dir_input.trim());
            let sqlite = dir.join("caliberate.db");
            if let Some(parent) = sqlite.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    self.preferences.set_error(err.to_string());
                    self.create_library_dialog_open = open;
                    return;
                }
            }
            if let Err(err) = self.switch_library(sqlite.display().to_string()) {
                self.preferences.set_error(err.to_string());
            } else {
                open = false;
            }
        }
        self.create_library_dialog_open = open;
    }

    fn switch_library(&mut self, sqlite_path: String) -> CoreResult<()> {
        if sqlite_path.trim().is_empty() {
            return Ok(());
        }
        let sqlite_path_buf = std::path::PathBuf::from(&sqlite_path);
        if let Some(parent) = sqlite_path_buf.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                caliberate_core::error::CoreError::Io(
                    "create library database parent".to_string(),
                    err,
                )
            })?;
        }
        info!(
            component = "gui_shell",
            sqlite_path = %sqlite_path_buf.display(),
            "switching library"
        );
        self.config.db.sqlite_path = sqlite_path_buf;
        self.library = LibraryView::new(&self.config)?;
        self.sync_library_layout();
        self.active_library_label = std::path::Path::new(&sqlite_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Library")
            .to_string();
        self.recent_libraries.retain(|entry| entry != &sqlite_path);
        self.recent_libraries.insert(0, sqlite_path);
        self.recent_libraries
            .truncate(self.recent_libraries_max.max(1));
        self.shell_config_dirty = true;
        Ok(())
    }

    fn sync_shell_config(&mut self) {
        if !self.shell_config_dirty {
            return;
        }
        self.config.gui.toolbar_icon_only = self.toolbar.icon_only;
        self.config.gui.toolbar_visible_actions = self.toolbar.to_actions();
        self.config.gui.global_search_scope = self.global_search_scope.as_config().to_string();
        self.config.gui.shortcut_preset = if self.shortcuts == ShortcutBindings::calibre_like() {
            "calibre_like".to_string()
        } else {
            "default".to_string()
        };
        self.config.gui.drag_drop_hints = self.drag_drop_hints_enabled;
        self.config.gui.command_palette_enabled = true;
        self.config.gui.notification_center_enabled = true;
        self.config.gui.mouse_gestures = self.mouse_gestures_enabled;
        self.config.gui.recent_libraries = self.recent_libraries.clone();
        self.config.gui.recent_libraries_max = self.recent_libraries_max;
        self.config.gui.active_library_label = self.active_library_label.clone();
        self.config.gui.pane_browser_visible = self.pane_browser_visible;
        self.config.gui.pane_browser_side = pane_side_to_config(self.pane_browser_side).to_string();
        self.config.gui.pane_details_visible = self.pane_details_visible;
        self.config.gui.pane_details_side = pane_side_to_config(self.pane_details_side).to_string();
        self.config.gui.pane_jobs_visible = self.pane_jobs_visible;
        self.config.gui.pane_left_width = self.pane_left_width;
        self.config.gui.pane_right_width = self.pane_right_width;
        self.config.gui.layout_preset = self.layout_preset.clone();
        self.config.gui.last_active_view = match self.active_view {
            AppView::Library => "library".to_string(),
            AppView::Preferences => "preferences".to_string(),
        };
        if self.recent_libraries.is_empty() {
            self.recent_libraries
                .push(self.config.db.sqlite_path.display().to_string());
        }
        if let Err(err) = self.config.save_to_path(&self.config_path) {
            self.library.request_refresh();
            self.preferences.set_error(err.to_string());
        } else {
            self.shell_config_dirty = false;
        }
    }
}
