use caliberate_core::config::ControlPlane;
use caliberate_core::logging;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = PathBuf::from("config/control-plane.toml");
    let config = ControlPlane::load_from_path(&config_path)?;
    let _logging_guard =
        logging::init(&config).map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;

    tracing::info!(component = "app", "caliberate startup complete");

    Ok(())
}
