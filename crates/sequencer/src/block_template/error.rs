use strata_db::DbError;
use strata_primitives::l2::L2BlockId;
use thiserror::Error;

/// Possible errors during block assembly and block template handling.
#[derive(Debug, Error)]
pub enum Error {
    /// Block generate was requested with timestamp earlier than acceptable.
    #[error("block timestamp too early: {0}")]
    TimestampTooEarly(u64),
    /// Request with an unknown template id.
    #[error("unknown templateid: {0}")]
    UnknownTemplateId(L2BlockId),
    /// Provided signature invalid for block template.
    #[error("invalid signature supplied for templateid: {0}")]
    InvalidSignature(L2BlockId),
    /// Could not send request to worker on channel due to rx being closed.
    #[error("failed to send request, template worker exited")]
    RequestChannelClosed,
    /// Could not receive response from worker on channel due to response tx being closed.
    #[error("failed to get response, template worker exited")]
    ResponseChannelClosed,
    /// Could not send message to FCM.
    #[error("failed to send fcm message, fcm worker exited")]
    FcmChannelClosed,
    /// Database Error.
    #[error("db: {0}")]
    DbError(#[from] DbError),
    /// Consensus Error.
    /// TODO: remove this and use local error variants
    #[error("consensus: {0}")]
    ConsensusError(#[from] strata_consensus_logic::errors::Error),
}
