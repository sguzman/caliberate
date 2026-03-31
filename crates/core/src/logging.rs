//! Tracing initialization.

use crate::error::{CoreError, CoreResult};
use tracing_subscriber::fmt;
use tracing_subscriber::EnvFilter;

pub fn init() -> CoreResult<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_line_number(true)
        .with_file(true)
        .try_init()
        .map_err(|err| CoreError::LoggingInit(err.to_string()))
}
