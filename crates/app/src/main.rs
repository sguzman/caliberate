use caliberate_core::logging;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init().map_err(|err| Box::new(err) as Box<dyn std::error::Error>)?;

    tracing::info!(component = "app", "caliberate startup complete");

    Ok(())
}
