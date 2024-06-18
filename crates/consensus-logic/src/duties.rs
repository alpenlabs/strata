//! Sequencer duties.

use std::time;

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_vertex_primitives::buf::Buf32;
use alpen_vertex_state::block::L2BlockId;

/// Describes when we'll stop working to fulfill a duty.
#[derive(Clone, Debug)]
pub enum Expiry {
    /// Duty expires when we see the next block.
    NextBlock,

    /// Duty expires when block is finalized to L1 in a batch.
    BlockFinalized,

    /// Duty expires after a certain timestamp.
    Timestamp(time::Instant),
}

/// Duties the sequencer might carry out.
#[derive(Clone, Debug)]
pub enum Duty {
    /// Goal to sign a block.
    SignBlock(BlockSigningDuty),
}

impl Duty {
    /// Returns when the duty should expire.
    pub fn expiry(&self) -> Expiry {
        match self {
            Self::SignBlock(_) => Expiry::NextBlock,
        }
    }
}

/// Describes information associated with signing a block.
#[derive(Clone, Debug)]
pub struct BlockSigningDuty {
    /// Slot to sign for.
    slot: u64,
}

impl BlockSigningDuty {
    pub fn new_simple(slot: u64) -> Self {
        Self { slot }
    }

    pub fn slot(&self) -> u64 {
        self.slot
    }
}

/// Manages a set of duties we need to carry out.
#[derive(Clone, Debug)]
pub struct DutyTracker {
    duties: Vec<DutyEntry>,
}

impl DutyTracker {
    /// Creates a new instance that has nothing in it.
    pub fn new_empty() -> Self {
        Self { duties: Vec::new() }
    }

    /// Returns the number of duties we still have to service.
    pub fn num_pending_duties(&self) -> usize {
        self.duties.len()
    }

    /// Updates the tracker with a new world state, purging relevant duties.
    pub fn update(&mut self, update: &StateUpdate) {
        let mut kept_duties = Vec::new();

        for d in self.duties.drain(..) {
            match d.duty.expiry() {
                Expiry::NextBlock => {
                    if d.created_slot < update.last_block_slot {
                        continue;
                    }
                }
                Expiry::BlockFinalized => {
                    if update.is_finalized(&d.created_blkid) {
                        continue;
                    }
                }
                Expiry::Timestamp(ts) => {
                    if update.cur_timestamp > ts {
                        continue;
                    }
                }
            }

            kept_duties.push(d);
        }

        self.duties = kept_duties
    }

    /// Returns an iterator over the currently live duties.
    pub fn duties_iter(&self) -> impl Iterator<Item = &Duty> {
        self.duties.iter().map(|de| &de.duty)
    }
}

#[derive(Clone, Debug)]
pub struct DutyEntry {
    duty: Duty,
    created_blkid: L2BlockId,
    created_slot: u64,
}

/// Describes an update to the world state which we use to expire some duties.
#[derive(Clone, Debug)]
pub struct StateUpdate {
    /// The slot we're currently at.
    last_block_slot: u64,

    /// The current timestamp we're currently at.
    cur_timestamp: time::Instant,

    /// Newly finalized blocks, must be sorted.
    newly_finalized_blocks: Vec<L2BlockId>,
}

impl StateUpdate {
    pub fn new(
        last_block_slot: u64,
        cur_timestamp: time::Instant,
        mut newly_finalized_blocks: Vec<L2BlockId>,
    ) -> Self {
        newly_finalized_blocks.sort();
        Self {
            last_block_slot,
            cur_timestamp,
            newly_finalized_blocks,
        }
    }

    pub fn new_simple(last_block_slot: u64, cur_timestamp: time::Instant) -> Self {
        Self::new(last_block_slot, cur_timestamp, Vec::new())
    }

    pub fn is_finalized(&self, id: &L2BlockId) -> bool {
        self.newly_finalized_blocks.binary_search(id).is_ok()
    }
}

/// Describes an identity that might be assigned duties.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum Identity {
    /// Sequencer with an identity key.
    Sequencer(Buf32),
}
