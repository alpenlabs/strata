//! Defines the error types associated with executing the withdrawal duties.

use alpen_express_btcio::rpc::error::ClientError as L1ClientError;
use express_bridge_tx_builder::errors::BridgeTxBuilderError;
use jsonrpsee::core::ClientError as L2ClientError;
use thiserror::Error;

/// Error during execution of the withdrawal duty.
#[derive(Error, Debug)]
pub enum ExecError {
    /// Error creating the [`TxSigningData`](alpen_express_primitives::bridge::TxSigningData).
    #[error("could not build withdraw transaction")]
    TxBuilder(#[from] BridgeTxBuilderError),

    /// Error while signing the withdrawal transaction.
    #[error("signing error: {0}")]
    Signing(String),

    /// The request for signature is invalid.
    #[error("invalid request")]
    InvalidRequest,

    /// Error while fetching a transaction state
    #[error("transaction state fetching error: {0}")]
    TxState(String),

    /// Error while broadcasting the signature/transaction.
    #[error("transaction broadcast error: {0}")]
    Broadcast(String),

    /// Error while processing withdrawal due to insufficient funds (for front-payments).
    #[error("insufficient funds")]
    InsufficientFunds,

    /// Unexpected error during the handling of the withdrawal.
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

/// Result of a execution that may produce an [`ExecError`].
pub type ExecResult<T> = Result<T, ExecError>;
