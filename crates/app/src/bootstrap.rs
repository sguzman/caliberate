use caliberate_core::config::ControlPlane;
use caliberate_core::{logging, metrics, paths};
use std::path::Path;

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
