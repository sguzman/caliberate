//! Plugin discovery and loading.

use crate::registry::{Plugin, PluginManifest, PluginRegistry};
use caliberate_core::config::PluginsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::Path;
use tracing::info;

const PLUGIN_MANIFEST: &str = "plugin.toml";

pub fn discover_plugins(config: &PluginsConfig) -> CoreResult<PluginRegistry> {
    let mut registry = PluginRegistry::new();
    if !config.enabled {
        return Ok(registry);
    }
    let root = &config.plugins_dir;
    if !root.exists() {
        return Ok(registry);
    }
    let entries = fs::read_dir(root)
        .map_err(|err| CoreError::Io(format!("read plugins dir {}", root.display()), err))?;
    for entry in entries {
        let entry = entry
            .map_err(|err| CoreError::Io(format!("read plugins dir {}", root.display()), err))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(plugin) = load_plugin(&path)? {
            registry.register(plugin);
        }
    }
    info!(
        component = "plugins",
        count = registry.list().len(),
        "plugins discovered"
    );
    Ok(registry)
}

fn load_plugin(path: &Path) -> CoreResult<Option<Plugin>> {
    let manifest_path = path.join(PLUGIN_MANIFEST);
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(&manifest_path)
        .map_err(|err| CoreError::Io("read plugin manifest".to_string(), err))?;
    let manifest: PluginManifest =
        toml::from_str(&content).map_err(|err| CoreError::ConfigParse(err.to_string()))?;
    let plugin = Plugin::from_manifest(path.to_path_buf(), manifest)?;
    Ok(Some(plugin))
}
