use caliberate_conversion::jobs::{ConversionJobRunner, build_request};
use caliberate_conversion::settings::ConversionSettings;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "ebook-convert", version, about = "Caliberate conversion CLI")]
struct EbookConvertCli {
    #[arg(long, default_value = "config/control-plane.toml")]
    config: PathBuf,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(index = 1)]
    input_pos: Option<PathBuf>,
    #[arg(index = 2)]
    output_pos: Option<PathBuf>,
    #[arg(long)]
    input_format: Option<String>,
    #[arg(long)]
    output_format: Option<String>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long)]
    max_input_bytes: Option<u64>,
    #[arg(long, default_value_t = false)]
    allow_passthrough: bool,
    #[arg(long, default_value_t = false)]
    disallow_passthrough: bool,
    #[arg(long, default_value_t = false)]
    dry_run: bool,
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

    let input = cli
        .input
        .or(cli.input_pos.clone())
        .ok_or("--input is required")?;
    let output = cli
        .output
        .or(cli.output_pos.clone())
        .ok_or("--output is required")?;

    if !config.conversion.enabled {
        return Err("conversion disabled by config".into());
    }

    if cli.allow_passthrough && cli.disallow_passthrough {
        return Err("cannot pass both --allow-passthrough and --disallow-passthrough".into());
    }

    let input = resolve_input(&input)?;
    let mut output = resolve_output(
        &input,
        &output,
        cli.output_dir.as_ref(),
        &config.conversion.output_dir,
    )?;
    let input_format = normalize_format(cli.input_format.clone()).or_else(|| infer_format(&input));
    let output_format =
        normalize_format(cli.output_format.clone()).or_else(|| infer_format(&output));

    let input_format = input_format.ok_or("input format missing")?;
    let output_format = output_format.ok_or("output format missing")?;

    ensure_supported_format(&input_format, &config.formats.supported)?;
    ensure_supported_format(&output_format, &config.formats.supported)?;

    if output.extension().is_none() {
        output = output.with_extension(&output_format);
    } else if !extension_matches_format(&output, &output_format) {
        return Err("output extension does not match output format".into());
    }

    ensure_output_not_directory(&output)?;
    ensure_distinct_paths(&input, &output)?;

    let mut settings = ConversionSettings::from_config(&config.conversion)
        .with_input_format(Some(input_format.clone()))
        .with_output_format(Some(output_format.clone()));
    if let Some(max_input_bytes) = cli.max_input_bytes {
        settings.max_input_bytes = max_input_bytes;
    }
    if cli.allow_passthrough {
        settings.allow_passthrough = true;
    }
    if cli.disallow_passthrough {
        settings.allow_passthrough = false;
    }

    info!(
        component = "conversion_cli",
        input = %input.display(),
        output = %output.display(),
        input_format = %input_format,
        output_format = %output_format,
        max_input_bytes = settings.max_input_bytes,
        allow_passthrough = settings.allow_passthrough,
        dry_run = cli.dry_run,
        "ebook-convert request"
    );

    if cli.dry_run {
        println!("Input: {}", input.display());
        println!("Output: {}", output.display());
        println!("Input format: {input_format}");
        println!("Output format: {output_format}");
        println!("Max input bytes: {}", settings.max_input_bytes);
        println!("Allow passthrough: {}", settings.allow_passthrough);
        return Ok(());
    }

    ensure_output_parent(&output)?;
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

fn resolve_input(input: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if !input.exists() {
        return Err(format!("input does not exist: {}", input.display()).into());
    }
    Ok(std::fs::canonicalize(input)?)
}

fn resolve_output(
    input: &PathBuf,
    output: &PathBuf,
    output_dir: Option<&PathBuf>,
    default_output_dir: &PathBuf,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_str = output.to_string_lossy();
    if output_str.starts_with('-') {
        return Err("output path cannot start with '-'".into());
    }
    if output_str.starts_with('.')
        && output_str.len() > 1
        && !output_str.contains(std::path::MAIN_SEPARATOR)
        && !output_str.contains('/')
        && !output_str.contains('\\')
    {
        let stem = input
            .file_stem()
            .ok_or("input missing file stem")?
            .to_string_lossy();
        let derived = format!("{stem}{output_str}");
        return Ok(resolve_output_dir(
            PathBuf::from(derived),
            output_dir,
            default_output_dir,
        )?);
    }
    resolve_output_dir(output.clone(), output_dir, default_output_dir)
}

fn infer_format(path: &PathBuf) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
}

fn ensure_supported_format(
    format: &str,
    supported: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let normalized = format.to_lowercase();
    if supported
        .iter()
        .any(|value| value.to_lowercase() == normalized)
    {
        Ok(())
    } else {
        Err(format!("unsupported format: {format}").into())
    }
}

fn normalize_format(value: Option<String>) -> Option<String> {
    value.map(|format| {
        let trimmed = format.trim_start_matches('.').to_string();
        trimmed.to_lowercase()
    })
}

fn resolve_output_dir(
    output: PathBuf,
    output_dir: Option<&PathBuf>,
    default_output_dir: &PathBuf,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(dir) = output_dir {
        let file_name = output.file_name().ok_or("output file missing name")?;
        return Ok(normalize_path(&dir.join(file_name))?);
    }
    if output.is_relative() {
        return Ok(normalize_path(&default_output_dir.join(output))?);
    }
    normalize_path(&output)
}

fn normalize_path(path: &PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if path.is_absolute() {
        return Ok(path.clone());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn extension_matches_format(path: &PathBuf, format: &str) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => ext.eq_ignore_ascii_case(format),
        None => true,
    }
}

fn ensure_output_not_directory(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() && path.is_dir() {
        return Err("output path is a directory".into());
    }
    Ok(())
}

fn ensure_distinct_paths(
    input: &PathBuf,
    output: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    if input == output {
        return Err("input and output paths are identical".into());
    }
    Ok(())
}

fn ensure_output_parent(output: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
