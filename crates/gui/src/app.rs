//! Application lifecycle wiring.

use eframe::egui;

#[derive(Default)]
pub struct CaliberateApp {
    status: String,
    books: Vec<String>,
    selected: Option<usize>,
}

impl CaliberateApp {
    pub fn new() -> Self {
        Self {
            status: "Ready".to_string(),
            books: vec!["Example Book".to_string(), "Second Book".to_string()],
            selected: None,
        }
    }
}

impl eframe::App for CaliberateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let available = ui.available_rect_before_wrap();
        let left_width = (available.width() * 0.3).max(220.0);

        egui::Panel::left("library_list")
            .resizable(true)
            .default_size(left_width)
            .show_inside(ui, |ui| {
                ui.heading("Library");
                ui.separator();
                for (idx, title) in self.books.iter().enumerate() {
                    let selected = self.selected == Some(idx);
                    if ui.selectable_label(selected, title).clicked() {
                        self.selected = Some(idx);
                    }
                }
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Details");
            ui.separator();
            match self.selected {
                Some(idx) => {
                    let title = &self.books[idx];
                    ui.label(format!("Title: {title}"));
                    ui.label("Format: epub");
                    ui.label(&self.status);
                }
                None => {
                    ui.label("Select a book to view details.");
                }
            }
        });
    }
}
