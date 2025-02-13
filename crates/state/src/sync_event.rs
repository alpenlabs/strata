use std::fmt;

use arbitrary::Arbitrary;
use async_trait::async_trait;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::l1::L1BlockCommitment;

use crate::l1::HeaderVerificationState;

/// Sync event that updates our consensus state.
#[derive(
    Clone, Debug, PartialEq, Eq, Arbitrary, BorshSerialize, BorshDeserialize, Deserialize, Serialize,
)]
pub enum SyncEvent {
    /// We've observed a valid L1 block.
    L1Block(L1BlockCommitment),

    /// Revert to a recent-ish L1 block.
    L1Revert(L1BlockCommitment),

    /// We've observed that the `genesis_l1_height` has reached maturity
    // FIXME this name doesn't make sense
    L1BlockGenesis(L1BlockCommitment, HeaderVerificationState),
}

impl fmt::Display for SyncEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::L1Block(block) => f.write_fmt(format_args!("l1block:{block:?}")),
            Self::L1Revert(block) => f.write_fmt(format_args!("l1revert:{block:?}")),
            // TODO implement this when we determine wwhat useful information we can take from here
            //Self::L1DABatch(h, _ckpts) => f.write_fmt(format_args!("l1da:<$data>@{h}")),
            Self::L1BlockGenesis(block, _st) => f.write_fmt(format_args!("l1genesis:{block:?}")),
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
