//! Device sync workflows.

use crate::detection::DeviceInfo;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone)]
pub struct DeviceSyncResult {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub bytes_copied: u64,
}

pub fn send_to_device(
    source: &Path,
    device: &DeviceInfo,
    dest_name: Option<&str>,
) -> CoreResult<DeviceSyncResult> {
    if !source.is_file() {
        return Err(CoreError::ConfigValidate(
            "source is not a file".to_string(),
        ));
    }
    fs::create_dir_all(&device.library_path)
        .map_err(|err| CoreError::Io("create device library dir".to_string(), err))?;
    let file_name = dest_name
        .map(|name| name.to_string())
        .or_else(|| {
            source
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .ok_or_else(|| CoreError::ConfigValidate("missing file name".to_string()))?;
    let dest = device.library_path.join(file_name);
    let bytes =
        fs::copy(source, &dest).map_err(|err| CoreError::Io("copy to device".to_string(), err))?;
    info!(
        component = "device",
        source = %source.display(),
        destination = %dest.display(),
        bytes,
        "sent to device"
    );
    Ok(DeviceSyncResult {
        source: source.to_path_buf(),
        destination: dest,
        bytes_copied: bytes,
    })
}

pub fn list_device_entries(device: &DeviceInfo) -> CoreResult<Vec<PathBuf>> {
    if !device.library_path.is_dir() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(&device.library_path).map_err(|err| {
        CoreError::Io(
            format!("read device library {}", device.library_path.display()),
            err,
        )
    })?;
    let mut results = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| {
            CoreError::Io(
                format!("read device entry {}", device.library_path.display()),
                err,
            )
        })?;
        let path = entry.path();
        if path.is_file() {
            results.push(path);
        }
    }
    Ok(results)
}

pub fn cleanup_device_orphans(device: &DeviceInfo, keep_files: &[String]) -> CoreResult<usize> {
    let mut removed = 0;
    let entries = list_device_entries(device)?;
    for path in entries {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if keep_files.iter().any(|keep| keep == name) {
            continue;
        }
        fs::remove_file(&path)
            .map_err(|err| CoreError::Io(format!("remove device file {}", path.display()), err))?;
        removed += 1;
    }
    info!(component = "device", removed, "cleaned device orphans");
    Ok(removed)
}
