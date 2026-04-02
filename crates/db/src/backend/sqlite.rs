//! SQLite connection management.

use caliberate_core::config::DbConfig;
use caliberate_core::error::{CoreError, CoreResult};
use rusqlite::Connection;
use rusqlite::functions::{Aggregate, Context, FunctionFlags};
use std::path::Path;
use tracing::info;
use uuid::Uuid;

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

    register_calibre_functions(&conn)?;

    info!(
        component = "db",
        path = %path.as_ref().display(),
        busy_timeout_ms,
        "sqlite connection opened"
    );

    Ok(conn)
}

fn register_calibre_functions(conn: &Connection) -> CoreResult<()> {
    conn.create_scalar_function(
        "title_sort",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let input: Option<String> = ctx.get(0)?;
            Ok(input.map(|value| title_sort(&value)))
        },
    )
    .map_err(|err| {
        CoreError::Io(
            "register title_sort function".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })?;

    conn.create_scalar_function(
        "books_list_filter",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |_ctx| Ok(1_i64),
    )
    .map_err(|err| {
        CoreError::Io(
            "register books_list_filter function".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })?;

    conn.create_scalar_function("uuid4", 0, FunctionFlags::SQLITE_UTF8, |_ctx| {
        Ok(Uuid::new_v4().to_string())
    })
    .map_err(|err| {
        CoreError::Io(
            "register uuid4 function".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })?;

    conn.create_aggregate_function(
        "concat",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        ConcatAggregate,
    )
    .map_err(|err| {
        CoreError::Io(
            "register concat aggregate".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })?;

    conn.create_aggregate_function(
        "sortconcat",
        2,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        SortConcatAggregate,
    )
    .map_err(|err| {
        CoreError::Io(
            "register sortconcat aggregate".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })?;

    Ok(())
}

fn title_sort(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    for prefix in ["the ", "a ", "an "] {
        if let Some(stripped) = lower.strip_prefix(prefix) {
            return stripped.trim().to_string();
        }
    }
    lower
}

struct ConcatAggregate;

impl Aggregate<Vec<String>, Option<String>> for ConcatAggregate {
    fn init(&self, _: &mut Context<'_>) -> rusqlite::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Vec<String>) -> rusqlite::Result<()> {
        let value: Option<String> = ctx.get(0)?;
        if let Some(value) = value {
            if !value.is_empty() {
                acc.push(value);
            }
        }
        Ok(())
    }

    fn finalize(
        &self,
        _: &mut Context<'_>,
        acc: Option<Vec<String>>,
    ) -> rusqlite::Result<Option<String>> {
        Ok(acc.and_then(|values| {
            if values.is_empty() {
                None
            } else {
                Some(values.join(", "))
            }
        }))
    }
}

struct SortConcatAggregate;

impl Aggregate<Vec<(i64, String)>, Option<String>> for SortConcatAggregate {
    fn init(&self, _: &mut Context<'_>) -> rusqlite::Result<Vec<(i64, String)>> {
        Ok(Vec::new())
    }

    fn step(&self, ctx: &mut Context<'_>, acc: &mut Vec<(i64, String)>) -> rusqlite::Result<()> {
        let order: Option<i64> = ctx.get(0)?;
        let value: Option<String> = ctx.get(1)?;
        if let (Some(order), Some(value)) = (order, value) {
            if !value.is_empty() {
                acc.push((order, value));
            }
        }
        Ok(())
    }

    fn finalize(
        &self,
        _: &mut Context<'_>,
        acc: Option<Vec<(i64, String)>>,
    ) -> rusqlite::Result<Option<String>> {
        let mut values = acc.unwrap_or_default();
        if values.is_empty() {
            return Ok(None);
        }
        values.sort_by_key(|entry| entry.0);
        Ok(Some(
            values
                .into_iter()
                .map(|entry| entry.1)
                .collect::<Vec<_>>()
                .join(", "),
        ))
    }
}
