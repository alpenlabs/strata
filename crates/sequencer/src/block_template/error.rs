use strata_db::DbError;
use strata_primitives::l2::L2BlockId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("block timestamp too early: {0}")]
    TimestampTooEarly(u64),
    #[error("unknown templateid: {0}")]
    UnknownTemplateId(L2BlockId),
    #[error("invalid signature supplied for templateid: {0})")]
    InvalidSignature(L2BlockId),
    #[error("db: {0}")]
    DbError(#[from] DbError),
    #[error("consensus: {0}")]
    ConsensusError(#[from] strata_consensus_logic::errors::Error),
    #[error("channel: {0}")]
    ChannelError(&'static str),
}
