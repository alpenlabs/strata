//! Defines the error types associated with executing the deposit duties.

use alpen_express_btcio::rpc::error::ClientError as L1ClientError;
use express_bridge_tx_builder::errors::BridgeTxBuilderError;
use jsonrpsee::core::ClientError as L2ClientError;
use thiserror::Error;

// TODO: use concrete types instead of passing around `String`

/// Error encountered during the deposit duty execution.
#[derive(Error, Debug)]
pub enum DepositExecError {
    /// Error creating the [`TxSigningData`](alpen_express_primitives::bridge::TxSigningData).
    #[error("could not build deposit transaction")]
    TxBuilder(#[from] BridgeTxBuilderError),

    /// Error occurred while signing a transaction.
    #[error("signing failed due to: {0}")]
    Signing(String),

    /// The request for signature is invalid.
    #[error("invalid request")]
    InvalidRequest,

    /// Error while fetching a transaction state
    #[error("transaction state fetching error: {0}")]
    TxState(String),

    /// Error occurred while broadcasting a message to the p2p network.
    #[error("transaction broadcast failed due to: {0}")]
    Broadcast(String),

    /// An unexpected error occurred during execution.
    #[error("execution failed: {0}")]
    Execution(String),

    /// Error communicating with the Bitcoin RPC.
    #[error("bitcoin RPC communication failed: {0}")]
    L1Rpc(#[from] L1ClientError),

    /// Error communicating with the rollup RPC.
    #[error("rollup RPC communication failed: {0}")]
    L2Rpc(#[from] L2ClientError),

    /// Signer does not have access to the [`Xpriv`](bitcoin::bip32::Xpriv)
    #[error("bitcoin signer do not have access to the private keys, i.e. xpriv")]
    Xpriv,
}

/// The result of a deposit duty execution which may produce a [`DepositExecError`].
pub type DepositExecResult<T> = Result<T, DepositExecError>;
