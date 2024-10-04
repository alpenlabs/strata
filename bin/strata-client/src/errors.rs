use std::io;

use alpen_express_primitives::params::ParamsError;
use format_serde_error::SerdeError;
use reth_rpc_types::engine::JwtError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum InitError {
    #[error("io: {0}")]
    Io(#[from] io::Error),

    #[error("config: {0}")]
    MalformedConfig(#[from] SerdeError),

    #[error("jwt: {0}")]
    MalformedSecret(#[from] JwtError),

    #[error("params: {0}")]
    MalformedParams(#[from] ParamsError),

    #[error("{0}")]
    Anyhow(#[from] anyhow::Error),
}
