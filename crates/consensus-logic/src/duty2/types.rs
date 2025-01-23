use strata_primitives::l2::L2BlockCommitment;
use strata_state::epoch::EpochCommitment;

pub enum SlotDuty {
    SignBlock(SignBlockContext),
}

/// Context we've extracted from the current world state to be able produce a new
/// block.
pub struct SignBlockContext {
    /// Target slot to produce the block in.
    pub slot: u64,

    /// Parent block.  It's possible this might not be the immediate preceeding
    /// slot when we decide to support skipped slots.
    pub parent: L2BlockCommitment,
}
