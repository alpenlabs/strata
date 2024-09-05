//! Defines errors related to book-keeping activities in the bridge client while executing bridge
//! duties.

use std::io;

use thiserror::Error;

/// Error encountered while reading/updating the checkpoint value.
#[derive(Error, Debug)]
pub enum TrackerError {
    /// IO error when interfacing with the underlying persistence layer for the checkpoint.
    #[error("storage error")]
    Io(#[from] io::Error),

    /// The data is incorrect or nor correctly formatted.
    #[error("invalid checkpoint data: {0}")]
    InvalidData(String),
}
