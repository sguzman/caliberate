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
    input: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    input_format: Option<String>,
    #[arg(long)]
    output_format: Option<String>,
    #[arg(long, default_value_t = false)]
    list_formats: bool,
    #[arg(long, default_value_t = false)]
    list_archives: bool,
    #[arg(long, default_value_t = false)]
    info: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = EbookConvertCli::parse();
    let bootstrap = caliberate_app::bootstrap::init(&cli.config)?;
    let config = bootstrap.config;

    if cli.list_formats {
        for format in &config.formats.supported {
            println!("{format}");
        }
        return Ok(());
    }

    if cli.list_archives {
        for format in &config.formats.archive_formats {
            println!("{format}");
        }
        return Ok(());
    }

    if cli.info {
        println!("Conversion enabled: {}", config.conversion.enabled);
        println!("Allow passthrough: {}", config.conversion.allow_passthrough);
        println!(
            "Default output format: {}",
            config.conversion.default_output_format
        );
        println!("Max input bytes: {}", config.conversion.max_input_bytes);
        println!("Temp dir: {}", config.conversion.temp_dir.display());
        println!("Output dir: {}", config.conversion.output_dir.display());
        println!("Supported formats: {}", config.formats.supported.join(", "));
        println!(
            "Archive formats: {}",
            config.formats.archive_formats.join(", ")
        );
        return Ok(());
    }

    let input = cli.input.ok_or("--input is required")?;
    let output = cli.output.ok_or("--output is required")?;

    if !config.conversion.enabled {
        return Err("conversion disabled by config".into());
    }

    let settings = ConversionSettings::from_config(&config.conversion)
        .with_input_format(cli.input_format)
        .with_output_format(cli.output_format);

    let runner = ConversionJobRunner::new();
    let request = build_request(&input, &output, settings);
    let summary = runner.run(request)?;

    println!(
        "Converted {} -> {} ({:?})",
        input.display(),
        output.display(),
        summary.duration
    );

    Ok(())
}
