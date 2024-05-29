//! Global consensus parameters for the rollup.

/// Consensus parameters that don't change for the lifetime of the network
/// (unless there's some weird hard fork).
#[derive(Clone, Debug)]
pub struct RollupParams {
    /// Block time in milliseconds.
    block_time: u64,
}

/// Client sync parameters that are used to make the network work but don't
/// strictly have to be pre-agreed.  These have to do with grace periods in
/// message delivery and whatnot.
#[derive(Clone, Debug)]
pub struct RunParams {
    /// Number of blocks that we follow the L1 from.
    l1_follow_distance: usize,
}

/// Combined set of parameters across all the consensus logic.
#[derive(Clone, Debug)]
pub struct Params {
    rollup: RollupParams,
    run: RunParams,
}

impl Params {
    pub fn rollup(&self) -> &RollupParams {
        &self.rollup
    }

    pub fn run(&self) -> &RunParams {
        &self.run
    }
}
