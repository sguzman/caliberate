//! Filesystem path policy helpers.

use crate::config::ControlPlane;
use crate::error::{CoreError, CoreResult};
use std::fs;
use std::path::Path;

pub fn ensure_runtime_paths(config: &ControlPlane) -> CoreResult<()> {
    ensure_dir(&config.paths.data_dir, "paths.data_dir")?;
    ensure_dir(&config.paths.cache_dir, "paths.cache_dir")?;
    ensure_dir(&config.paths.log_dir, "paths.log_dir")?;
    ensure_dir(&config.paths.tmp_dir, "paths.tmp_dir")?;
    ensure_dir(&config.paths.library_dir, "paths.library_dir")?;
    ensure_dir(&config.conversion.temp_dir, "conversion.temp_dir")?;
    ensure_dir(&config.conversion.output_dir, "conversion.output_dir")?;
    ensure_dir(&config.conversion.job_logs_dir, "conversion.job_logs_dir")?;
    ensure_dir(&config.news.recipes_dir, "news.recipes_dir")?;
    ensure_dir(&config.news.downloads_dir, "news.downloads_dir")?;

    if let Some(parent) = config.db.sqlite_path.parent() {
        ensure_dir(parent, "db.sqlite_path parent")?;
    }
    if let Some(parent) = config.conversion.job_history_path.parent() {
        ensure_dir(parent, "conversion.job_history_path parent")?;
    }
    if let Some(parent) = config.news.history_path.parent() {
        ensure_dir(parent, "news.history_path parent")?;
    }

    Ok(())
}

fn ensure_dir(path: &Path, label: &'static str) -> CoreResult<()> {
    fs::create_dir_all(path).map_err(|err| CoreError::Io(format!("create {label}"), err))?;
    tracing::info!(component = "paths", path = %path.display(), "ensured directory");
    Ok(())
}
