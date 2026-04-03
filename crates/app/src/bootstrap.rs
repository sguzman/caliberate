use caliberate_core::config::ControlPlane;
use caliberate_core::{logging, metrics, paths};
use std::path::Path;

pub struct BootstrapState {
    pub config: ControlPlane,
    _logging_guard: logging::LoggingGuard,
    _metrics: metrics::MetricsHandle,
}

pub fn init<P: AsRef<Path>>(path: P) -> Result<BootstrapState, Box<dyn std::error::Error>> {
    let config = ControlPlane::load_from_path(path)?;
    let logging_guard = logging::init(&config)?;
    paths::ensure_runtime_paths(&config)?;
    let metrics = metrics::init(&config);

    Ok(BootstrapState {
        config,
        _logging_guard: logging_guard,
        _metrics: metrics,
    })
}
