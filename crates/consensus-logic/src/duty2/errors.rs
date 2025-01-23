use strata_primitives::l2::L2BlockId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// This is used in cases like if not all of the blocks in an epoch have
    /// been produced, for example.
    #[error("preexisting condition for determining duty not met")]
    NotReady,

    #[error("missing chainstate for block {0}")]
    MissingChainstate(L2BlockId),
}
