use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "caliberate-gui", version, about = "Caliberate GUI")]
struct GuiCli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = GuiCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    caliberate_gui::run(bootstrap.config, cli.config)
        .map_err(|err| Box::new(err) as Box<dyn std::error::Error>)
}
