//! Metadata extraction workflows.

use caliberate_core::config::FormatsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct BasicMetadata {
    pub title: String,
    pub format: String,
    pub file_size: u64,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ArchivePreview {
    pub format: String,
    pub entries: Vec<String>,
}

pub fn extract_basic(path: &Path, formats: &FormatsConfig) -> CoreResult<BasicMetadata> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| CoreError::ConfigValidate("file extension missing".to_string()))?;

    if !formats.supported.iter().any(|fmt| fmt == &extension)
        && !formats.archive_formats.iter().any(|fmt| fmt == &extension)
    {
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

pub fn extract_archive_preview(path: &Path, formats: &FormatsConfig) -> CoreResult<ArchivePreview> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .ok_or_else(|| CoreError::ConfigValidate("file extension missing".to_string()))?;

    if !formats.archive_formats.iter().any(|fmt| fmt == &extension) {
        return Err(CoreError::ConfigValidate(format!(
            "unsupported archive format: {extension}"
        )));
    }

    if extension != "zip" {
        return Err(CoreError::ConfigValidate(format!(
            "archive format not supported yet: {extension}"
        )));
    }

    let file = fs::File::open(path)
        .map_err(|err| CoreError::Io("open archive".to_string(), err))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let mut entries = Vec::new();

    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        entries.push(file.name().to_string());
    }

    Ok(ArchivePreview {
        format: extension,
        entries,
    })
}
