//! Plugin registration and lifecycle.

use crate::sandbox::{Permission, Permissions};
use caliberate_core::error::{CoreError, CoreResult};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub entrypoint: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub permissions: Permissions,
}

#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: Vec<Plugin>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: Plugin) {
        self.plugins.push(plugin);
    }

    pub fn list(&self) -> &[Plugin] {
        &self.plugins
    }

    pub fn find(&self, name: &str) -> Option<&Plugin> {
        self.plugins
            .iter()
            .find(|plugin| plugin.manifest.name == name)
    }
}

impl Plugin {
    pub fn from_manifest(path: PathBuf, manifest: PluginManifest) -> CoreResult<Self> {
        let permissions = Permissions::from_strings(&manifest.permissions)?;
        Ok(Self {
            manifest,
            path,
            permissions,
        })
    }
}

impl Permission {
    pub fn parse(value: &str) -> CoreResult<Self> {
        match value {
            "files.read" => Ok(Permission::FilesRead),
            "files.write" => Ok(Permission::FilesWrite),
            "network" => Ok(Permission::Network),
            "exec" => Ok(Permission::Exec),
            "device" => Ok(Permission::Device),
            _ => Err(CoreError::ConfigValidate(format!(
                "unknown plugin permission: {value}"
            ))),
        }
    }
}
