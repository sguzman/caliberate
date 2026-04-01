//! Metadata extraction workflows.

use caliberate_core::config::FormatsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_zpaq::extract_unmodeled_file;
use sevenz_rust2::{ArchiveReader, Password};
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use tempfile::Builder;
use tracing::info;
use unrar::Archive as RarArchive;
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

    let entries = match extension.as_str() {
        "zip" => list_zip_entries(path)?,
        "7z" => list_7z_entries(path)?,
        "rar" => list_rar_entries(path)?,
        "zpaq" => list_zpaq_entries(path)?,
        _ => {
            return Err(CoreError::ConfigValidate(format!(
                "archive format not supported yet: {extension}"
            )));
        }
    };

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

    let output_path = match extension.as_str() {
        "zip" => extract_zip_entry(path, entry_name, output_dir)?,
        "7z" => extract_7z_entry(path, entry_name, output_dir)?,
        "rar" => extract_rar_entry(path, entry_name, output_dir)?,
        "zpaq" => extract_zpaq_entry(path, entry_name, output_dir)?,
        _ => {
            return Err(CoreError::ConfigValidate(format!(
                "archive format not supported yet: {extension}"
            )));
        }
    };

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

fn list_zip_entries(path: &Path) -> CoreResult<Vec<String>> {
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

    Ok(entries)
}

fn list_7z_entries(path: &Path) -> CoreResult<Vec<String>> {
    let reader = ArchiveReader::open(path, Password::empty())
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let entries = reader
        .archive()
        .files
        .iter()
        .filter(|entry| !entry.is_directory)
        .map(|entry| entry.name.clone())
        .collect();
    Ok(entries)
}

fn list_rar_entries(path: &Path) -> CoreResult<Vec<String>> {
    let mut entries = Vec::new();
    let archive = RarArchive::new(path)
        .open_for_listing()
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    for entry in archive {
        let header = entry.map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        if header.is_file() {
            entries.push(header.filename.to_string_lossy().into_owned());
        }
    }
    Ok(entries)
}

fn list_zpaq_entries(path: &Path) -> CoreResult<Vec<String>> {
    let segments = extract_unmodeled_file(path)?;
    let entries = segments
        .into_iter()
        .map(|segment| segment.filename)
        .collect();
    Ok(entries)
}

fn extract_zip_entry(path: &Path, entry_name: &str, output_dir: &Path) -> CoreResult<PathBuf> {
    let file =
        fs::File::open(path).map_err(|err| CoreError::Io("open archive".to_string(), err))?;
    let mut archive =
        ZipArchive::new(file).map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;

    let entry_path = sanitize_archive_entry(entry.name())?;
    let output_path = output_dir.join(entry_path);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create archive output dir".to_string(), err))?;
    }

    let mut output = fs::File::create(&output_path)
        .map_err(|err| CoreError::Io("create extracted file".to_string(), err))?;
    std::io::copy(&mut entry, &mut output)
        .map_err(|err| CoreError::Io("extract archive entry".to_string(), err))?;
    Ok(output_path)
}

fn extract_7z_entry(path: &Path, entry_name: &str, output_dir: &Path) -> CoreResult<PathBuf> {
    let safe_entry = sanitize_archive_entry(entry_name)?;
    let output_path = output_dir.join(&safe_entry);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create archive output dir".to_string(), err))?;
    }
    let mut reader = ArchiveReader::open(path, Password::empty())
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    let data = reader
        .read_file(entry_name)
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    fs::write(&output_path, data)
        .map_err(|err| CoreError::Io("write extracted entry".to_string(), err))?;
    Ok(output_path)
}

fn extract_rar_entry(path: &Path, entry_name: &str, output_dir: &Path) -> CoreResult<PathBuf> {
    let safe_entry = sanitize_archive_entry(entry_name)?;
    let output_path = output_dir.join(&safe_entry);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create archive output dir".to_string(), err))?;
    }

    let mut archive = RarArchive::new(path)
        .open_for_processing()
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    loop {
        let next = archive
            .read_header()
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
        let Some(open) = next else {
            break;
        };
        let entry = open.entry();
        if entry.is_file() && entry.filename.to_string_lossy() == entry_name {
            open.extract_to(&output_path)
                .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
            return Ok(output_path);
        }
        archive = open
            .skip()
            .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;
    }

    Err(CoreError::ConfigValidate(format!(
        "archive entry not found: {entry_name}"
    )))
}

fn extract_zpaq_entry(path: &Path, entry_name: &str, output_dir: &Path) -> CoreResult<PathBuf> {
    let safe_entry = sanitize_archive_entry(entry_name)?;
    let output_path = output_dir.join(&safe_entry);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create archive output dir".to_string(), err))?;
    }

    let segments = extract_unmodeled_file(path)?;
    let segment = segments
        .into_iter()
        .find(|segment| segment.filename == entry_name)
        .ok_or_else(|| {
            CoreError::ConfigValidate(format!("archive entry not found: {entry_name}"))
        })?;
    fs::write(&output_path, segment.data)
        .map_err(|err| CoreError::Io("write extracted entry".to_string(), err))?;
    Ok(output_path)
}

fn sanitize_archive_entry(entry_name: &str) -> CoreResult<PathBuf> {
    let entry_path = Path::new(entry_name);
    if entry_path.is_absolute()
        || entry_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(CoreError::ConfigValidate(
            "archive entry contains invalid path".to_string(),
        ));
    }
    Ok(entry_path.to_path_buf())
}
