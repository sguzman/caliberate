//! Device detection and enumeration.

use caliberate_core::config::DeviceConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub mount_path: PathBuf,
    pub library_path: PathBuf,
}

pub fn detect_devices(config: &DeviceConfig) -> CoreResult<Vec<DeviceInfo>> {
    let mut devices = Vec::new();
    for root in &config.mount_roots {
        if !root.exists() {
            continue;
        }
        let entries = fs::read_dir(root).map_err(|err| {
            CoreError::Io(format!("read device mount root {}", root.display()), err)
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                CoreError::Io(format!("read device entry {}", root.display()), err)
            })?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let library_path = path.join(&config.library_subdir);
            if library_path.is_dir() {
                let name = entry.file_name().to_string_lossy().trim().to_string();
                devices.push(DeviceInfo {
                    name,
                    mount_path: path,
                    library_path,
                });
            }
        }
    }
    info!(
        component = "device",
        count = devices.len(),
        "detected devices"
    );
    Ok(devices)
}

pub fn device_library_path(device: &DeviceInfo) -> &Path {
    device.library_path.as_path()
}
