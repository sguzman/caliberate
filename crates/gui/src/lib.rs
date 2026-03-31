//! GUI application shell and views.

pub mod app;
pub mod preferences;
pub mod views;

pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Caliberate",
        options,
        Box::new(|_cc| Ok(Box::new(app::CaliberateApp::new()))),
    )
}
