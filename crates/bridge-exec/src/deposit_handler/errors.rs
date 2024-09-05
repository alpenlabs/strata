//! Defines the error types associated with executing the deposit duties.

use thiserror::Error;

// TODO: use concrete types instead of passing around `String`

/// Error encountered during the deposit duty execution.
#[derive(Error, Debug)]
pub enum DepositExecError {
    /// Error occurred while signing a transaction.
    #[error("signing failed due to: {0}")]
    Signing(String),

    /// The request for signature is invalid.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Error occurred while broadcasting a message to the p2p network.
    #[error("transaction broadcast failed due to: {0}")]
    Broadcast(String),

    /// An unexpected error occurred during execution.
    #[error("execution failed: {0}")]
    Execution(String),
}

/// The result of a deposit duty execution which may produce a [`DepositExecError`].
pub type DepositExecResult<T> = Result<T, DepositExecError>;
