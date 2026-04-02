//! GUI application shell and views.

pub mod app;
pub mod preferences;
pub mod views;

pub fn run(
    config: caliberate_core::config::ControlPlane,
    config_path: std::path::PathBuf,
) -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Caliberate",
        options,
        Box::new(|_cc| {
            let app = app::CaliberateApp::try_new(config, config_path)
                .map_err(|err| eframe::Error::AppCreation(Box::new(err)))?;
            Ok(Box::new(app))
        }),
    )
}
