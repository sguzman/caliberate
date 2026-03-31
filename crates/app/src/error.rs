use caliberate_core::error::CoreError;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::time::Duration;

#[derive(Debug)]
pub enum AppError {
    Core(CoreError),
    RuntimeInit(io::Error),
    Signal(io::Error),
    ShutdownTimeout(Duration),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Core(err) => write!(f, "core error: {err}"),
            AppError::RuntimeInit(err) => write!(f, "failed to initialize runtime: {err}"),
            AppError::Signal(err) => write!(f, "failed to install signal handler: {err}"),
            AppError::ShutdownTimeout(duration) => write!(
                f,
                "shutdown did not complete within {}ms",
                duration.as_millis()
            ),
        }
    }
}

impl StdError for AppError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            AppError::Core(err) => Some(err),
            AppError::RuntimeInit(err) => Some(err),
            AppError::Signal(err) => Some(err),
            AppError::ShutdownTimeout(_) => None,
        }
    }
}

impl From<CoreError> for AppError {
    fn from(value: CoreError) -> Self {
        AppError::Core(value)
    }
}

pub type AppResult<T> = Result<T, AppError>;
