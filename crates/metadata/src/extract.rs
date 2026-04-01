//! Metadata extraction workflows.

use caliberate_core::config::FormatsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use tempfile::Builder;
use tracing::info;
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

    let metadata =
        fs::metadata(path).map_err(|err| CoreError::Io("read metadata file".to_string(), err))?;
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

    let file =
        fs::File::open(path).map_err(|err| CoreError::Io("open archive".to_string(), err))?;
    let mut archive =
        ZipArchive::new(file).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
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

pub fn extract_archive_entry(
    path: &Path,
    entry_name: &str,
    output_dir: &Path,
    formats: &FormatsConfig,
) -> CoreResult<PathBuf> {
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

    let file =
        fs::File::open(path).map_err(|err| CoreError::Io("open archive".to_string(), err))?;
    let mut archive =
        ZipArchive::new(file).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;

    let entry_path = Path::new(entry.name());
    if entry_path.is_absolute()
        || entry_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CoreError::ConfigValidate(
            "archive entry contains invalid path".to_string(),
        ));
    }

    let output_path = output_dir.join(entry_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create archive output dir".to_string(), err))?;
    }

    let mut output = fs::File::create(&output_path)
        .map_err(|err| CoreError::Io("create extracted file".to_string(), err))?;
    std::io::copy(&mut entry, &mut output)
        .map_err(|err| CoreError::Io("extract archive entry".to_string(), err))?;

    info!(
        component = "metadata",
        archive = %path.display(),
        entry = %entry_name,
        dest = %output_path.display(),
        "archive entry extracted"
    );

    Ok(output_path)
}

pub fn extract_archive_entry_to_temp(
    path: &Path,
    entry_name: &str,
    tmp_dir: &Path,
    formats: &FormatsConfig,
) -> CoreResult<PathBuf> {
    let temp_dir = Builder::new()
        .prefix("caliberate-archive-")
        .tempdir_in(tmp_dir)
        .map_err(|err| CoreError::Io("create temp archive dir".to_string(), err))?;
    let temp_path = temp_dir.keep();
    extract_archive_entry(path, entry_name, &temp_path, formats)
}
