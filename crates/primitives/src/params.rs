//! Global consensus parameters for the rollup.

use crate::block_credential::CredRule;

/// Consensus parameters that don't change for the lifetime of the network
/// (unless there's some weird hard fork).
#[derive(Clone, Debug)]
pub struct RollupParams {
    /// Block time in milliseconds.
    pub block_time: u64,

    /// Rule we use to decide if a block is correctly signed.
    pub cred_rule: CredRule,

    /// Block height from which to watch for L1 transactions
    pub l1_start_block_height: u64,
}

/// Client sync parameters that are used to make the network work but don't
/// strictly have to be pre-agreed.  These have to do with grace periods in
/// message delivery and whatnot.
#[derive(Clone, Debug)]
pub struct RunParams {
    /// Number of blocks that we follow the L1 from.
    pub l1_follow_distance: u64,
}

/// Combined set of parameters across all the consensus logic.
#[derive(Clone, Debug)]
pub struct Params {
    pub rollup: RollupParams,
    pub run: RunParams,
}

impl Params {
    pub fn rollup(&self) -> &RollupParams {
        &self.rollup
    }

    pub fn run(&self) -> &RunParams {
        &self.run
    }
}
