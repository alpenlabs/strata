use strata_state::{l1::L1VerificationError, prelude::*};
use thiserror::Error;

/// Errors for block state transition.
#[derive(Debug, Error)]
pub enum TsnError {
    #[error("skipped a block")]
    SkippedBlock,

    #[error("mismatch parent (head {0:?}, parent {1:?}")]
    MismatchParent(L2BlockId, L2BlockId),

    #[error("attested mismatched ID for {0} (set {1}, computed {2})")]
    L1BlockIdMismatch(u64, L1BlockId, L1BlockId),

    #[error("parent link at L1 block {0} incorrect (set parent {1}, found block {2})")]
    L1BlockParentMismatch(u64, L1BlockId, L1BlockId),

    #[error("L1 segment block did not extend the chain tip")]
    L1SegNotExtend,

    #[error("ran out of deposits to assign withdrawals to")]
    InsufficientDepositsForIntents,

    #[error("there are no operators in the chainstate")]
    NoOperators,

    #[error("applied el ops and el ops from chain state doesn't match")]
    ElOpsMismatch,

    /// Indicates an error occurred during the verification of an L1 block.
    ///
    /// This variant wraps the underlying [`L1VerificationError`] that provides details about the
    /// failure.
    #[error("L1 block verification failed: {0}")]
    L1BlockVerification(#[from] L1VerificationError),
}
