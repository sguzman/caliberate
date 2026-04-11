//! GUI application shell and views.

pub mod app;
pub mod preferences;
pub mod views;

pub fn run(
    config: caliberate_core::config::ControlPlane,
    config_path: std::path::PathBuf,
) -> Result<(), eframe::Error> {
    tracing::info!(
        component = "gui_shell",
        window_restore = config.gui.window_restore,
        width = config.gui.window_width,
        height = config.gui.window_height,
        x = config.gui.window_pos_x,
        y = config.gui.window_pos_y,
        "launching GUI runtime"
    );
    let viewport = if config.gui.window_restore {
        eframe::egui::ViewportBuilder::default()
            .with_inner_size([config.gui.window_width, config.gui.window_height])
            .with_position([config.gui.window_pos_x, config.gui.window_pos_y])
    } else {
        eframe::egui::ViewportBuilder::default()
    };
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
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
