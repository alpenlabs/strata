//! Defines the error types associated with executing the withdrawal duties.

use thiserror::Error;

/// Error during execution of the withdrawal duty.
#[derive(Error, Debug)]
pub enum WithdrawalExecError {
    /// Error while signing the withdrawal transaction.
    #[error("signing error: {0}")]
    Signing(String),

    /// Error produced if the withdrawal request is invalid.
    #[error("invalid request")]
    InvalidRequest,

    /// Error while broadcasting the signature/transaction.
    #[error("transaction broadcast error: {0}")]
    Broadcast(String),

    /// Error while processing withdrawal due to insufficient funds (for front-payments).
    #[error("insufficient funds")]
    InsufficientFunds,

    /// Unexpected error during the handling of the withdrawal.
    #[error("execution failed: {0}")]
    Execution(String),
}

/// Result of a withdrawal execution that may produce an [`WithdrawalExecError`].
pub type WithdrawalExecResult<T> = Result<T, WithdrawalExecError>;
