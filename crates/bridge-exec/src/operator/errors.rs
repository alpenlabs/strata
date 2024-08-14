//! Defines error types that may occur during operator duty execution.
use thiserror::Error;

use crate::{
    deposit_handler::errors::DepositExecError, withdrawal_handler::errors::WithdrawalExecError,
};

/// Error during execution of bridge duties.
#[derive(Error, Debug)]
pub enum ExecError {
    /// Error during execution of the bridge-in (deposit) duty.
    #[error("deposit error: {0}")]
    DepositError(#[from] DepositExecError),

    /// Error during execution of the bridge-out (withdrawal) duty.
    #[error("withdrawal error: {0}")]
    WithdrawalError(#[from] WithdrawalExecError),

    /// Unexpected error during execution of a duty.
    #[error("unknown job type: {0}")]
    UnknownJobType(String),
}

/// Result of a bridge duty execution that may produce an [`ExecError`].
pub type ExecResult<T> = Result<T, ExecError>;
