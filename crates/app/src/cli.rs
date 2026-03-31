use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "caliberate", version, about = "Caliberate core application")]
pub struct Cli {
    #[arg(long, default_value = "config/control-plane.toml")]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    CheckConfig,
}
