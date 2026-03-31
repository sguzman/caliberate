//! Tracing initialization.

use crate::config::{AppMode, ControlPlane};
use crate::error::{CoreError, CoreResult};
use std::fs;
use time::format_description;
use time::OffsetDateTime;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::EnvFilter;

pub struct LoggingGuard {
    _file_guard: Option<tracing_appender::non_blocking::WorkerGuard>,
}

pub fn init(config: &ControlPlane) -> CoreResult<LoggingGuard> {
    let filter = EnvFilter::try_from_env("RUST_LOG")
        .unwrap_or_else(|_| EnvFilter::new(config.logging.level.clone()));

    let stdout_writer = if config.logging.stdout {
        tracing_subscriber::fmt::writer::BoxMakeWriter::new(std::io::stdout)
    } else {
        tracing_subscriber::fmt::writer::BoxMakeWriter::new(std::io::sink)
    };

    let mut guard = LoggingGuard { _file_guard: None };
    let file_writer = if config.app.mode == AppMode::Dev || config.logging.file_enabled {
        let (writer, file_guard) = open_log_writer(config)?;
        guard._file_guard = Some(file_guard);
        tracing_subscriber::fmt::writer::BoxMakeWriter::new(writer)
    } else {
        tracing_subscriber::fmt::writer::BoxMakeWriter::new(std::io::sink)
    };

    let combined_writer = stdout_writer.and(file_writer);

    let base_builder = tracing_subscriber::fmt::Subscriber::builder()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_line_number(true)
        .with_file(true)
        .with_writer(combined_writer);

    if config.logging.json {
        base_builder
            .json()
            .try_init()
            .map_err(|err| CoreError::LoggingInit(err.to_string()))?;
    } else {
        base_builder
            .try_init()
            .map_err(|err| CoreError::LoggingInit(err.to_string()))?;
    }

    Ok(guard)
}

fn open_log_writer(
    config: &ControlPlane,
) -> CoreResult<(
    tracing_appender::non_blocking::NonBlocking,
    tracing_appender::non_blocking::WorkerGuard,
)> {
    fs::create_dir_all(&config.paths.log_dir)
        .map_err(|err| CoreError::Io("create log dir".to_string(), err))?;

    let timestamp = format_timestamp()?;
    let file_path = config
        .paths
        .log_dir
        .join(format!("caliberate-{timestamp}.log"));
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|err| CoreError::Io(format!("open log file {}", file_path.display()), err))?;

    Ok(tracing_appender::non_blocking(file))
}

fn format_timestamp() -> CoreResult<String> {
    let now = OffsetDateTime::now_utc();
    let format = format_description::parse("[year][month][day]-[hour][minute][second]")
        .map_err(|err| CoreError::ConfigParse(err.to_string()))?;
    now.format(&format)
        .map_err(|err| CoreError::ConfigParse(err.to_string()))
}
