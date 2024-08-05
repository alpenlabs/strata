//! Sequencer duties.

use std::time;

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::id::L2BlockId;

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

    pub fn target_slot(&self) -> u64 {
        self.slot
    }
}

/// Manages a set of duties we need to carry out.
#[derive(Clone, Debug)]
pub struct DutyTracker {
    next_id: u64,
    duties: Vec<DutyEntry>,
    finalized_block: Option<L2BlockId>,
}

impl DutyTracker {
    /// Creates a new instance that has nothing in it.
    pub fn new_empty() -> Self {
        Self {
            next_id: 1,
            duties: Vec::new(),
            finalized_block: None,
        }
    }

    /// Returns the number of duties we still have to service.
    pub fn num_pending_duties(&self) -> usize {
        self.duties.len()
    }

    /// Updates the tracker with a new world state, purging relevant duties.
    pub fn update(&mut self, update: &StateUpdate) -> usize {
        let mut kept_duties = Vec::new();

        if update.latest_finalized_block.is_some() {
            self.set_finalized_block(update.latest_finalized_block);
        }

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

        let old_cnt = self.duties.len();
        self.duties = kept_duties;
        self.duties.len() - old_cnt
    }

    /// Adds some more duties.
    pub fn add_duties(&mut self, blkid: L2BlockId, slot: u64, duties: impl Iterator<Item = Duty>) {
        self.duties.extend(duties.map(|d| DutyEntry {
            duty: d,
            id: {
                // This is horrible but it works. :)
                let id = self.next_id;
                self.next_id += 1;
                id
            },
            created_blkid: blkid,
            created_slot: slot,
        }));
    }

    pub fn set_finalized_block(&mut self, blkid: Option<L2BlockId>) {
        self.finalized_block = blkid;
    }

    pub fn get_finalized_block(&self) -> Option<L2BlockId> {
        self.finalized_block
    }

    /// Returns the slice of duties we're keeping around.
    pub fn duties(&self) -> &[DutyEntry] {
        &self.duties
    }
}

#[derive(Clone, Debug)]
pub struct DutyEntry {
    /// Duty data itself.
    duty: Duty,

    /// ID used to help avoid re-performing a duty.
    id: u64,

    /// Block ID it was created for.
    created_blkid: L2BlockId,

    /// Slot it was created for.
    created_slot: u64,
}

impl DutyEntry {
    pub fn duty(&self) -> &Duty {
        &self.duty
    }

    pub fn id(&self) -> u64 {
        self.id
    }
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

    /// Latest finalized block.
    latest_finalized_block: Option<L2BlockId>,
}

impl StateUpdate {
    pub fn new(
        last_block_slot: u64,
        cur_timestamp: time::Instant,
        mut newly_finalized_blocks: Vec<L2BlockId>,
    ) -> Self {
        // Extract latest finalized block before sorting
        let latest_finalized_block = newly_finalized_blocks.first().cloned();

        newly_finalized_blocks.sort();

        Self {
            last_block_slot,
            cur_timestamp,
            newly_finalized_blocks,
            latest_finalized_block,
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

#[derive(Clone, Debug)]
pub struct DutyBatch {
    sync_ev_idx: u64,
    duties: Vec<DutyEntry>,
}

impl DutyBatch {
    pub fn new(sync_ev_idx: u64, duties: Vec<DutyEntry>) -> Self {
        Self {
            sync_ev_idx,
            duties,
        }
    }

    pub fn sync_ev_idx(&self) -> u64 {
        self.sync_ev_idx
    }

    pub fn duties(&self) -> &[DutyEntry] {
        &self.duties
    }
}
