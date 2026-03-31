//! SQLite connection management.

use caliberate_core::config::DbConfig;
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::Connection;
use std::path::Path;
use tracing::info;

pub fn open(config: &DbConfig) -> CoreResult<Connection> {
    open_with_timeout(&config.sqlite_path, config.busy_timeout_ms)
}

pub fn open_with_timeout<P: AsRef<Path>>(path: P, busy_timeout_ms: u64) -> CoreResult<Connection> {
    let conn = Connection::open(path.as_ref()).map_err(|err| {
        CoreError::Io(
            "open sqlite connection".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })?;

    conn.busy_timeout(std::time::Duration::from_millis(busy_timeout_ms))
        .map_err(|err| {
            CoreError::Io(
                "set sqlite busy timeout".to_string(),
                std::io::Error::new(std::io::ErrorKind::Other, err),
            )
        })?;

    info!(
        component = "db",
        path = %path.as_ref().display(),
        busy_timeout_ms,
        "sqlite connection opened"
    );

    Ok(conn)
}
