//! Application lifecycle wiring.

use caliberate_core::error::CoreResult;
use eframe::egui;

use crate::preferences::PreferencesView;
use crate::views::LibraryView;

pub struct CaliberateApp {
    config: caliberate_core::config::ControlPlane,
    library: LibraryView,
    preferences: PreferencesView,
    active_view: AppView,
}

impl CaliberateApp {
    pub fn try_new(config: caliberate_core::config::ControlPlane) -> CoreResult<Self> {
        let library = LibraryView::new(&config)?;
        let preferences = PreferencesView::new();
        Ok(Self {
            config,
            library,
            preferences,
            active_view: AppView::Library,
        })
    }
}

impl eframe::App for CaliberateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_nav").show_inside(ui, |ui| {
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
                self.library.ui(ui);
            }
            AppView::Preferences => {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.preferences.ui(ui, &self.config);
                });
            }
        }

        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            let (status, error) = self.library.status_line();
            if let Some(err) = error {
                ui.colored_label(egui::Color32::from_rgb(190, 0, 0), err);
            } else {
                ui.label(status);
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppView {
    Library,
    Preferences,
}
