use std::io;

use alloy_rpc_types::engine::JwtError;
use format_serde_error::SerdeError;
use strata_primitives::params::ParamsError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("missing init client state")]
    MissingInitClientState,

    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("unparsable params file: {0}")]
    UnparsableParamsFile(#[from] SerdeError),

    #[error("config: {0:?}")]
    MalformedConfig(#[from] ConfigError),

    #[error("jwt: {0}")]
    MalformedSecret(#[from] JwtError),

    #[error("params: {0}")]
    MalformedParams(#[from] ParamsError),

    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    /// Config is not a toml table.
    #[error("Config is not a toml table")]
    ConfigNotTomlTable,

    /// Missing key in table.
    #[error("missing key: {0}")]
    MissingKey(String),

    /// Tried to traverse into a primitive.
    #[error("can't traverse into primitive: {0}")]
    TraversePrimitiveAt(String),

    /// Bad override string.
    #[error("malformed override string")]
    MalformedOverrideStr,

    #[error("Config is not parseable")]
    ConfigNotParseable,

    #[error("Invalid override: '{0}'")]
    InvalidOverride(String),
}
