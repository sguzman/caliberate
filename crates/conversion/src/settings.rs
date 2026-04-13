//! Conversion settings and defaults.

use caliberate_core::config::ConversionConfig;

#[derive(Debug, Clone)]
pub struct ConversionSettings {
    pub input_format: Option<String>,
    pub output_format: String,
    pub allow_passthrough: bool,
    pub max_input_bytes: u64,
    pub input_profile: String,
    pub output_profile: String,
    pub heuristic_enable: bool,
    pub heuristic_unwrap_lines: bool,
    pub heuristic_delete_blank_lines: bool,
    pub page_margin_left: f32,
    pub page_margin_right: f32,
    pub page_margin_top: f32,
    pub page_margin_bottom: f32,
    pub embed_fonts: bool,
    pub subset_fonts: bool,
    pub cover_policy: String,
}

impl ConversionSettings {
    pub fn from_config(config: &ConversionConfig) -> Self {
        Self {
            input_format: None,
            output_format: config.default_output_format.clone(),
            allow_passthrough: config.allow_passthrough,
            max_input_bytes: config.max_input_bytes,
            input_profile: config.default_input_profile.clone(),
            output_profile: config.default_output_profile.clone(),
            heuristic_enable: config.heuristic_enable,
            heuristic_unwrap_lines: config.heuristic_unwrap_lines,
            heuristic_delete_blank_lines: config.heuristic_delete_blank_lines,
            page_margin_left: config.page_margin_left,
            page_margin_right: config.page_margin_right,
            page_margin_top: config.page_margin_top,
            page_margin_bottom: config.page_margin_bottom,
            embed_fonts: config.embed_fonts,
            subset_fonts: config.subset_fonts,
            cover_policy: config.cover_policy.clone(),
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

    pub fn with_profiles(mut self, input_profile: String, output_profile: String) -> Self {
        self.input_profile = input_profile;
        self.output_profile = output_profile;
        self
    }

    pub fn with_heuristics(
        mut self,
        heuristic_enable: bool,
        heuristic_unwrap_lines: bool,
        heuristic_delete_blank_lines: bool,
    ) -> Self {
        self.heuristic_enable = heuristic_enable;
        self.heuristic_unwrap_lines = heuristic_unwrap_lines;
        self.heuristic_delete_blank_lines = heuristic_delete_blank_lines;
        self
    }

    pub fn with_page_setup(
        mut self,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
        embed_fonts: bool,
        subset_fonts: bool,
    ) -> Self {
        self.page_margin_left = left;
        self.page_margin_right = right;
        self.page_margin_top = top;
        self.page_margin_bottom = bottom;
        self.embed_fonts = embed_fonts;
        self.subset_fonts = subset_fonts;
        self
    }

    pub fn with_cover_policy(mut self, cover_policy: String) -> Self {
        self.cover_policy = cover_policy;
        self
    }
}
