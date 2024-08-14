//! Sequencer duties.

use std::time;

use borsh::{BorshDeserialize, BorshSerialize};

use alpen_express_primitives::{buf::Buf32, hash::compute_borsh_hash};
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

    /// Duty expires after a specific L2 block is finalized
    BlockIdFinalized(L2BlockId),
}

/// Duties the sequencer might carry out.
#[derive(Clone, Debug, BorshSerialize)]
pub enum Duty {
    /// Goal to sign a block.
    SignBlock(BlockSigningDuty),
    /// Goal to write batch data to L1
    CommitBatch(BatchCommitmentDuty),
}

impl Duty {
    /// Returns when the duty should expire.
    pub fn expiry(&self) -> Expiry {
        match self {
            Self::SignBlock(_) => Expiry::NextBlock,
            Self::CommitBatch(BatchCommitmentDuty { blockid, .. }) => {
                Expiry::BlockIdFinalized(*blockid)
            }
        }
    }

    pub fn id(&self) -> Buf32 {
        compute_borsh_hash(self)
    }
}

/// Describes information associated with signing a block.
#[derive(Clone, Debug, BorshSerialize)]
pub struct BlockSigningDuty {
    /// Slot to sign for.
    slot: u64,
    /// Parent to build on
    parent: L2BlockId,
}

impl BlockSigningDuty {
    pub fn new_simple(slot: u64, parent: L2BlockId) -> Self {
        Self { slot, parent }
    }

    pub fn target_slot(&self) -> u64 {
        self.slot
    }

    pub fn parent(&self) -> L2BlockId {
        self.parent
    }
}

#[derive(Debug, Clone, BorshSerialize)]
pub struct BatchCommitmentDuty {
    /// Last slot of batch
    slot: u64,
    /// Id of block in last slot
    blockid: L2BlockId,
}

impl BatchCommitmentDuty {
    pub fn new(slot: u64, blockid: L2BlockId) -> Self {
        Self { slot, blockid }
    }

    pub fn end_slot(&self) -> u64 {
        self.slot
    }
}

/// Manages a set of duties we need to carry out.
#[derive(Clone, Debug)]
pub struct DutyTracker {
    duties: Vec<DutyEntry>,
    finalized_block: Option<L2BlockId>,
}

impl DutyTracker {
    /// Creates a new instance that has nothing in it.
    pub fn new_empty() -> Self {
        Self {
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

        let old_cnt = self.duties.len();
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
                Expiry::BlockIdFinalized(l2blockid) => {
                    if update.is_finalized(&l2blockid) {
                        continue;
                    }
                }
            }

            kept_duties.push(d);
        }

        self.duties = kept_duties;
        old_cnt - self.duties.len()
    }

    /// Adds some more duties.
    pub fn add_duties(&mut self, blkid: L2BlockId, slot: u64, duties: impl Iterator<Item = Duty>) {
        self.duties.extend(duties.map(|d| DutyEntry {
            id: d.id(),
            duty: d,
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
    id: Buf32,

    /// Block ID it was created for.
    created_blkid: L2BlockId,

    /// Slot it was created for.
    created_slot: u64,
}

impl DutyEntry {
    pub fn duty(&self) -> &Duty {
        &self.duty
    }

    pub fn id(&self) -> Buf32 {
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

/// Sequencer key used for signing-related duties.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum IdentityKey {
    Sequencer(Buf32),
}

/// Contains both the identity key used for signing and the identity used for
/// verifying signatures.  This is really just a stub that we should replace
/// with real cryptographic signatures and putting keys in the rollup params.
#[derive(Clone, Debug)]
pub struct IdentityData {
    pub ident: Identity,
    pub key: IdentityKey,
}

impl IdentityData {
    pub fn new(ident: Identity, key: IdentityKey) -> Self {
        Self { ident, key }
    }
}
