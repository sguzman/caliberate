//! Conversion pipeline stages.

use crate::formats;
use crate::settings::ConversionSettings;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::info;

#[derive(Debug, Clone)]
pub struct ConversionReport {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub input_format: String,
    pub output_format: String,
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub duration: Duration,
}

pub fn convert_file(
    input: &Path,
    output: &Path,
    settings: &ConversionSettings,
) -> CoreResult<ConversionReport> {
    let started = Instant::now();
    let metadata =
        fs::metadata(input).map_err(|err| CoreError::Io("read input metadata".to_string(), err))?;
    if metadata.len() > settings.max_input_bytes {
        return Err(CoreError::ConfigValidate(
            "input exceeds conversion.max_input_bytes".to_string(),
        ));
    }

    let input_format = settings
        .input_format
        .clone()
        .or_else(|| {
            input
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_lowercase())
        })
        .ok_or_else(|| CoreError::ConfigValidate("input format missing".to_string()))?;

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create output dir".to_string(), err))?;
    }

    let output_bytes = formats::convert_file(input, output, settings, &input_format)?;

    let report = ConversionReport {
        input_path: input.to_path_buf(),
        output_path: output.to_path_buf(),
        input_format: input_format.clone(),
        output_format: settings.output_format.clone(),
        input_bytes: metadata.len(),
        output_bytes,
        duration: started.elapsed(),
    };

    info!(
        component = "conversion",
        input = %report.input_path.display(),
        output = %report.output_path.display(),
        input_format = %report.input_format,
        output_format = %report.output_format,
        input_bytes = report.input_bytes,
        output_bytes = report.output_bytes,
        duration_ms = report.duration.as_millis(),
        "conversion completed"
    );

    Ok(report)
}
