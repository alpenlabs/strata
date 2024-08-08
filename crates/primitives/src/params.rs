//! Global consensus parameters for the rollup.

use crate::{block_credential::CredRule, prelude::Buf32};

/// Consensus parameters that don't change for the lifetime of the network
/// (unless there's some weird hard fork).
#[derive(Clone, Debug)]
pub struct RollupParams {
    /// Block time in milliseconds.
    pub block_time: u64,

    /// Rule we use to decide if a block is correctly signed.
    pub cred_rule: CredRule,

    /// Block height from which to watch for L1 transactions
    pub horizon_l1_height: u64,

    /// Block height we'll construct the L2 genesis block from.
    pub genesis_l1_height: u64,

    /// Hardcoded EL genesis info
    /// TODO: move elsewhere
    pub evm_genesis_block_hash: Buf32,
    pub evm_genesis_block_state_root: Buf32,

    /// Depth after which we consider the L1 block to not reorg
    pub l1_reorg_safe_depth: u32,
}

/// Client sync parameters that are used to make the network work but don't
/// strictly have to be pre-agreed.  These have to do with grace periods in
/// message delivery and whatnot.
#[derive(Clone, Debug)]
pub struct RunParams {
    /// Number of blocks that we follow the L1 from.
    pub l1_follow_distance: u64,
    /// Number of events after which we checkpoint the client
    pub client_checkpoint_interval: u32,
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
