//! Core error types.

use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum CoreError {
    LoggingInit(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::LoggingInit(message) => {
                write!(f, "failed to initialize logging: {message}")
            }
        }
    }
}

impl StdError for CoreError {}

pub type CoreResult<T> = Result<T, CoreError>;
