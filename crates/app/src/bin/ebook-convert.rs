use caliberate_conversion::jobs::{ConversionJobRunner, build_request};
use caliberate_conversion::settings::ConversionSettings;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "ebook-convert", version, about = "Caliberate conversion CLI")]
struct EbookConvertCli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: PathBuf,
    #[arg(long)]
    input: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    input_format: Option<String>,
    #[arg(long)]
    output_format: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = EbookConvertCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    let config = bootstrap.config;

    if !config.conversion.enabled {
        return Err("conversion disabled by config".into());
    }

    let settings = ConversionSettings::from_config(&config.conversion)
        .with_input_format(cli.input_format)
        .with_output_format(cli.output_format);

    let runner = ConversionJobRunner::new();
    let request = build_request(&cli.input, &cli.output, settings);
    let summary = runner.run(request)?;

    println!(
        "Converted {} -> {} ({:?})",
        cli.input.display(),
        cli.output.display(),
        summary.duration
    );

    Ok(())
}
