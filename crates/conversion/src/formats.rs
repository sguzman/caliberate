//! Format-specific conversion logic.

use crate::settings::ConversionSettings;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::Path;

pub fn convert_file(
    input: &Path,
    output: &Path,
    settings: &ConversionSettings,
    input_format: &str,
) -> CoreResult<u64> {
    if input_format == settings.output_format {
        if !settings.allow_passthrough {
            return Err(CoreError::ConfigValidate(
                "passthrough conversion disabled".to_string(),
            ));
        }
        let bytes = fs::copy(input, output)
            .map_err(|err| CoreError::Io("copy passthrough output".to_string(), err))?;
        return Ok(bytes);
    }

    Err(CoreError::ConfigValidate(format!(
        "converter not implemented: {input_format} -> {}",
        settings.output_format
    )))
}
