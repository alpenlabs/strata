//! Defines the configuration parameters for the bridge client that need to supplied externally by
//! the user running it.

use core::time;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use alpen_express_primitives::l1::BitcoinAddress;
use bitcoin::Network;
use format_serde_error::SerdeError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The configuration for the bridge client that is supplied by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The details of the reserved bitcoin address that the bridge client must use to service
    /// withdrawals.
    pub reserved_addr: AddressConfig,

    /// The path to private data required for authorization.
    pub secrets: SecretsConfig,

    /// The frequency with which the bridge client queries the full node (in secs).
    pub sync_interval: time::Duration,
    // TODO: add other configuration options such as those related to status reporting, key/wallet
    // management, etc.
}

/// The configuration for the bitcoin address used by the operator to front-pay users during
/// withdrawal fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressConfig {
    /// The bitcoin address.
    pub address: BitcoinAddress,

    /// The network type associated with the bitcoin address.
    pub network: Network,
}

impl Config {
    /// Parse the config at the given path and produce the [`Config`].
    pub fn load_from_path(path: impl AsRef<Path>) -> InitResult<Self> {
        let contents = fs::read_to_string(path)?;
        let config = toml::from_str::<Config>(contents.as_str())
            .map_err(|e| SerdeError::new(contents, e))?;

        Ok(config)
    }
}

/// The details required for authorization activities (such as signing).
// TODO: find a better way to manage keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// The path to the file that contains the operator private key.
    private_key: PathBuf,

    /// The path to the file that contains the operator public key.
    pub_key: PathBuf,
}

/// Error during initialization.
// TODO: move this out to a dedicated module or `bin`.
#[derive(Debug, Error)]
pub enum InitError {
    /// I/O related error while reading config.
    #[error("error loading config file: {0}")]
    Io(#[from] io::Error),

    /// Error while parsing the provided config.
    #[error("invalid config data: {0}")]
    MalformedConfig(#[from] SerdeError),
}

/// Result of parsing the config file which may produce a [`ConfigError`].
pub type InitResult<T> = Result<T, InitError>;
