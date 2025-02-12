use std::fmt;

use arbitrary::Arbitrary;
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::l1::L1BlockCommitment;

use crate::{
    batch::L1CommittedCheckpoint,
    l1::{HeaderVerificationState, L1BlockId},
};

/// Sync event that updates our consensus state.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Deserialize, Serialize,
)]
pub enum SyncEvent {
    /// We've observed a valid L1 block.
    L1Block(u64, L1BlockId),

    /// Revert to a recent-ish L1 block.
    L1Revert(L1BlockCommitment),

    /// New checkpoint posted to L1 in a DA batch at given height.
    // FIXME what does this data mean?
    L1DABatch(u64, Vec<L1CommittedCheckpoint>),

    /// We've observed that the `genesis_l1_height` has reached maturity
    L1BlockGenesis(u64, HeaderVerificationState),
}

impl fmt::Display for SyncEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::L1Block(h, id) => f.write_fmt(format_args!("l1block:{id}@{h}")),
            Self::L1Revert(h) => f.write_fmt(format_args!("l1revert:{h:?}")),
            // TODO implement this when we determine wwhat useful information we can take from here
            Self::L1DABatch(h, _ckpts) => f.write_fmt(format_args!("l1da:<$data>@{h}")),
            Self::L1BlockGenesis(h, _st) => f.write_fmt(format_args!("l1genesis:{h}")),
        }
    }
}

/// Interface to submit event to CSM in blocking or async fashion.
// TODO reverse the convention on these function names, since you can't
// accidentally call an async fn in a blocking context
#[async_trait]
pub trait EventSubmitter {
    /// Submit event blocking
    fn submit_event(&self, sync_event: SyncEvent) -> anyhow::Result<()>;
    /// Submit event async
    async fn submit_event_async(&self, sync_event: SyncEvent) -> anyhow::Result<()>;
}
