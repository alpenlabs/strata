//! Errors associated with the Exec configuration.

use std::io;

use format_serde_error::SerdeError;
use thiserror::Error;

/// Error while reading config.
// TODO: make this a part of an `InitError` to avoid having hyper-specific error types.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// I/O related error while reading config.
    #[error("error loading config file: {0}")]
    Io(#[from] io::Error),

    /// Error while parsing the provided config.
    #[error("invalid config data: {0}")]
    MalformedConfig(#[from] SerdeError),
}

/// Result of parsing the config file which may produce a [`ConfigError`].
pub type ConfigResult<T> = Result<T, ConfigError>;
