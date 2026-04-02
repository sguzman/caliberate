//! Plugin sandboxing and permissions.

use caliberate_core::error::{CoreError, CoreResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    FilesRead,
    FilesWrite,
    Network,
    Exec,
    Device,
}

#[derive(Debug, Clone)]
pub struct Permissions {
    pub allows_files_read: bool,
    pub allows_files_write: bool,
    pub allows_network: bool,
    pub allows_exec: bool,
    pub allows_device: bool,
}

impl Permissions {
    pub fn from_strings(values: &[String]) -> CoreResult<Self> {
        let mut permissions = Self {
            allows_files_read: false,
            allows_files_write: false,
            allows_network: false,
            allows_exec: false,
            allows_device: false,
        };
        for value in values {
            match value.as_str() {
                "files.read" => permissions.allows_files_read = true,
                "files.write" => permissions.allows_files_write = true,
                "network" => permissions.allows_network = true,
                "exec" => permissions.allows_exec = true,
                "device" => permissions.allows_device = true,
                _ => {
                    return Err(CoreError::ConfigValidate(format!(
                        "unknown plugin permission: {value}"
                    )));
                }
            }
        }
        Ok(permissions)
    }
}
