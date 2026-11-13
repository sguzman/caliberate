use caliberate_core::config::ControlPlane;
use caliberate_core::{logging, metrics, paths};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Choose the installed control-plane file, never the checkout's example config.
///
/// Command-line --config overrides this default in each entry point.
pub fn default_config_path() -> PathBuf {
    resolve_default_config_path(
        std::env::var_os("CALIBERATE_CONFIG").as_deref(),
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("APPDATA").as_deref(),
    )
}

fn resolve_default_config_path(
    config_override: Option<&OsStr>,
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
    appdata: Option<&OsStr>,
) -> PathBuf {
    if let Some(path) = config_override.filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    if let Some(path) = xdg_config_home.filter(|path| !path.is_empty()) {
        let root = PathBuf::from(path);
        if root.is_absolute() {
            return root.join("caliberate/control-plane.toml");
        }
    }
    if let Some(path) = home.filter(|path| !path.is_empty()) {
        return Path::new(path).join(".config/caliberate/control-plane.toml");
    }
    if let Some(path) = appdata.filter(|path| !path.is_empty()) {
        return Path::new(path).join("caliberate/control-plane.toml");
    }
    // No home-directory environment available: fail at load rather than silently
    // loading config/control-plane.toml from a source checkout.
    PathBuf::from(".config/caliberate/control-plane.toml")
}

pub struct BootstrapState {
    pub config: ControlPlane,
    _logging_guard: logging::LoggingGuard,
    _metrics: metrics::MetricsHandle,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BootstrapOptions {
    pub stdout_logging: Option<bool>,
}

pub fn init<P: AsRef<Path>>(path: P) -> Result<BootstrapState, Box<dyn std::error::Error>> {
    init_with_options(path, BootstrapOptions::default())
}

pub fn init_with_options<P: AsRef<Path>>(
    path: P,
    options: BootstrapOptions,
) -> Result<BootstrapState, Box<dyn std::error::Error>> {
    let config = ControlPlane::load_from_path(path)?;
    let logging_guard = logging::init_with_stdout(
        &config,
        options.stdout_logging.unwrap_or(config.logging.stdout),
    )?;
    paths::ensure_runtime_paths(&config)?;
    let metrics = metrics::init(&config);

    Ok(BootstrapState {
        config,
        _logging_guard: logging_guard,
        _metrics: metrics,
    })
}

#[cfg(test)]
mod config_path_tests {
    use super::resolve_default_config_path;
    use std::ffi::OsStr;
    use std::path::PathBuf;

    #[test]
    fn override_is_preferred_over_installed_locations() {
        assert_eq!(
            resolve_default_config_path(
                Some(OsStr::new("/custom/caliberate.toml")),
                Some(OsStr::new("/xdg")),
                Some(OsStr::new("/home/alice")),
                None,
            ),
            PathBuf::from("/custom/caliberate.toml"),
        );
    }

    #[test]
    fn xdg_config_home_is_used_when_absolute() {
        let xdg = std::env::temp_dir().join("caliberate-config-test");
        assert_eq!(
            resolve_default_config_path(None, Some(xdg.as_os_str()), None, None),
            xdg.join("caliberate/control-plane.toml"),
        );
    }

    #[test]
    fn home_config_is_used_when_xdg_is_unset_or_relative() {
        assert_eq!(
            resolve_default_config_path(
                None,
                Some(OsStr::new("relative-xdg")),
                Some(OsStr::new("/home/alice")),
                None,
            ),
            PathBuf::from("/home/alice/.config/caliberate/control-plane.toml"),
        );
    }

    #[test]
    fn appdata_fallback_when_home_unset() {
        assert_eq!(
            resolve_default_config_path(None, None, None, Some(OsStr::new("appdata"))),
            PathBuf::from("appdata/caliberate/control-plane.toml"),
        );
    }

    #[test]
    fn repo_example_is_never_the_implicit_default() {
        assert_eq!(
            resolve_default_config_path(None, None, None, None),
            PathBuf::from(".config/caliberate/control-plane.toml"),
        );
    }
}
