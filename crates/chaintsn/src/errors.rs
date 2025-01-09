use strata_state::prelude::*;
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

    #[error("block missing L1 segment when expected")]
    ExpectedL1Segment,

    #[error("block had L1 segment when not expected")]
    ExpectedNoL1Segment,

    #[error("there are no operators in the chainstate")]
    NoOperators,

    #[error("applied el ops and el ops from chain state doesn't match")]
    ElOpsMismatch,
}
