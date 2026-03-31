//! Application lifecycle wiring.

use eframe::egui;

#[derive(Default)]
pub struct CaliberateApp {
    status: String,
}

impl CaliberateApp {
    pub fn new() -> Self {
        Self {
            status: "Ready".to_string(),
        }
    }
}

impl eframe::App for CaliberateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Caliberate");
        ui.label(&self.status);
    }
}
