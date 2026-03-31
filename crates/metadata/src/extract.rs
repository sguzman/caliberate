//! Metadata extraction workflows.

use caliberate_core::config::FormatsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BasicMetadata {
    pub title: String,
    pub format: String,
    pub file_size: u64,
    pub source_path: PathBuf,
}

pub fn extract_basic(path: &Path, formats: &FormatsConfig) -> CoreResult<BasicMetadata> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| CoreError::ConfigValidate("file extension missing".to_string()))?;

    if !formats.supported.iter().any(|fmt| fmt == &extension) {
        return Err(CoreError::ConfigValidate(format!(
            "unsupported format: {extension}"
        )));
    }

    let metadata = fs::metadata(path)
        .map_err(|err| CoreError::Io("read metadata file".to_string(), err))?;
    let title = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(BasicMetadata {
        title,
        format: extension,
        file_size: metadata.len(),
        source_path: path.to_path_buf(),
    })
}
