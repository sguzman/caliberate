//! Conversion settings and defaults.

use caliberate_core::config::ConversionConfig;

#[derive(Debug, Clone)]
pub struct ConversionSettings {
    pub input_format: Option<String>,
    pub output_format: String,
    pub allow_passthrough: bool,
    pub max_input_bytes: u64,
}

impl ConversionSettings {
    pub fn from_config(config: &ConversionConfig) -> Self {
        Self {
            input_format: None,
            output_format: config.default_output_format.clone(),
            allow_passthrough: config.allow_passthrough,
            max_input_bytes: config.max_input_bytes,
        }
    }

    pub fn with_input_format(mut self, input_format: Option<String>) -> Self {
        self.input_format = input_format;
        self
    }

    pub fn with_output_format(mut self, output_format: Option<String>) -> Self {
        if let Some(format) = output_format {
            self.output_format = format;
        }
        self
    }
}
