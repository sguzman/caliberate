use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "caliberate", version, about = "Caliberate core application")]
pub struct Cli {
    #[arg(long, default_value_os_t = caliberate_app::bootstrap::default_config_path())]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    CheckConfig,
}
