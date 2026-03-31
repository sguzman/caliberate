//! Core error types.

use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CoreError {
    ConfigLoad(PathBuf, io::Error),
    ConfigParse(String),
    ConfigValidate(String),
    Io(String, io::Error),
    LoggingInit(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::ConfigLoad(path, err) => {
                write!(f, "failed to load config from {}: {err}", path.display())
            }
            CoreError::ConfigParse(message) => {
                write!(f, "failed to parse config: {message}")
            }
            CoreError::ConfigValidate(message) => {
                write!(f, "invalid config: {message}")
            }
            CoreError::Io(context, err) => {
                write!(f, "io error during {context}: {err}")
            }
            CoreError::LoggingInit(message) => {
                write!(f, "failed to initialize logging: {message}")
            }
        }
    }
}

impl StdError for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
