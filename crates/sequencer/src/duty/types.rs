//! Sequencer duties.

use std::{collections::HashSet, time};

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, hash::compute_borsh_hash};
use strata_state::{
    batch::{BatchInfo, BatchTransition, BootstrapState},
    id::L2BlockId,
};

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

    /// Duty expires after a specific checkpoint is finalized on bitcoin
    CheckpointIdxFinalized(u64),
}

/// Duties the sequencer might carry out.
#[derive(Clone, Debug, BorshSerialize, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Duty {
    /// Goal to sign a block.
    SignBlock(BlockSigningDuty),
    /// Goal to build and commit a batch.
    CommitBatch(BatchCheckpointDuty),
}

impl Duty {
    /// Returns when the duty should expire.
    pub fn expiry(&self) -> Expiry {
        match self {
            Self::SignBlock(_) => Expiry::NextBlock,
            Self::CommitBatch(duty) => Expiry::CheckpointIdxFinalized(duty.idx()),
        }
    }

    /// Returns a unique identifier for the duty.
    pub fn id(&self) -> Buf32 {
        match self {
            // We want Batch commitment duty to be unique by the checkpoint idx
            Self::CommitBatch(duty) => compute_borsh_hash(&duty.idx()),
            _ => compute_borsh_hash(self),
        }
    }
}

/// Describes information associated with signing a block.
#[derive(Clone, Debug, BorshSerialize, Serialize, Deserialize)]
pub struct BlockSigningDuty {
    /// Slot to sign for.
    slot: u64,
    /// Parent to build on
    parent: L2BlockId,
    /// Target timestamp for block
    target_ts: u64,
}

impl BlockSigningDuty {
    /// Create new block signing duty from components.
    pub fn new_simple(slot: u64, parent: L2BlockId, target_ts: u64) -> Self {
        Self {
            slot,
            parent,
            target_ts,
        }
    }

    /// Returns target slot for block signing duty.
    pub fn target_slot(&self) -> u64 {
        self.slot
    }

    /// Returns parent block id for block signing duty.
    pub fn parent(&self) -> L2BlockId {
        self.parent
    }

    /// Returns target ts for block signing duty.
    pub fn target_ts(&self) -> u64 {
        self.target_ts
    }
}

/// This duty is created whenever a previous batch is found on L1 and verified.
/// When this duty is created, in order to execute the duty, the sequencer looks for corresponding
/// batch proof in the proof db.
#[derive(Clone, Debug, BorshSerialize, Serialize, Deserialize)]
pub struct BatchCheckpointDuty {
    /// Checkpoint/batch info
    batch_info: BatchInfo,

    /// Checkpoint/batch transition which needs to be proven
    batch_transition: BatchTransition,

    /// Bootstrapping state based on which the `batch_transition` will be verified
    bootstrap_state: BootstrapState,
}

impl BatchCheckpointDuty {
    pub fn new(
        batch_info: BatchInfo,
        batch_transition: BatchTransition,
        bootstrap_state: BootstrapState,
    ) -> Self {
        Self {
            batch_info,
            batch_transition,
            bootstrap_state,
        }
    }

    /// Gen checkpoint index.
    pub fn idx(&self) -> u64 {
        self.batch_info.epoch()
    }

    pub fn batch_info(&self) -> &BatchInfo {
        &self.batch_info
    }

    pub fn batch_transition(&self) -> &BatchTransition {
        &self.batch_transition
    }

    pub fn bootstrap_state(&self) -> &BootstrapState {
        &self.bootstrap_state
    }
}

/// Manages a set of duties we need to carry out.
#[derive(Clone, Debug)]
pub struct DutyTracker {
    duties: Vec<DutyEntry>,
    duty_ids: HashSet<Buf32>,
    finalized_block: Option<L2BlockId>,
}

impl DutyTracker {
    /// Creates a new instance that has nothing in it.
    pub fn new_empty() -> Self {
        Self {
            duties: Vec::new(),
            duty_ids: HashSet::new(),
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
        let mut duty_ids = HashSet::new();

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
                Expiry::CheckpointIdxFinalized(idx) => {
                    if update
                        .latest_finalized_batch
                        .filter(|&x| x >= idx)
                        .is_some()
                    {
                        continue;
                    }
                }
            }

            duty_ids.insert(d.id());
            kept_duties.push(d);
        }

        self.duties = kept_duties;
        self.duty_ids = duty_ids;
        old_cnt - self.duties.len()
    }

    /// Adds some more duties.
    pub fn add_duties(&mut self, blkid: L2BlockId, slot: u64, duties: impl Iterator<Item = Duty>) {
        self.duties.extend(duties.filter_map(|duty| {
            let id = duty.id();
            if self.duty_ids.contains(&id) {
                return None;
            }

            Some(DutyEntry {
                id,
                duty,
                created_blkid: blkid,
                created_slot: slot,
            })
        }));
    }

    /// Sets the finalized block.
    pub fn set_finalized_block(&mut self, blkid: Option<L2BlockId>) {
        self.finalized_block = blkid;
    }

    /// Get finalized block.
    pub fn get_finalized_block(&self) -> Option<L2BlockId> {
        self.finalized_block
    }

    /// Returns the slice of duties we're keeping around.
    pub fn duties(&self) -> &[DutyEntry] {
        &self.duties
    }
}

/// Represents a single duty inside duty tracker.
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
    /// Get reference to Duty.
    pub fn duty(&self) -> &Duty {
        &self.duty
    }

    /// Get duty ID.
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

    /// Latest finalized batch.
    latest_finalized_batch: Option<u64>,
}

impl StateUpdate {
    /// Create a new state update.
    pub fn new(
        last_block_slot: u64,
        cur_timestamp: time::Instant,
        mut newly_finalized_blocks: Vec<L2BlockId>,
        latest_finalized_batch: Option<u64>,
    ) -> Self {
        // Extract latest finalized block before sorting
        let latest_finalized_block = newly_finalized_blocks.first().cloned();

        newly_finalized_blocks.sort();

        Self {
            last_block_slot,
            cur_timestamp,
            newly_finalized_blocks,
            latest_finalized_block,
            latest_finalized_batch,
        }
    }

    /// Create state update without blocks or batch info.
    pub fn new_simple(last_block_slot: u64, cur_timestamp: time::Instant) -> Self {
        Self::new(last_block_slot, cur_timestamp, Vec::new(), None)
    }

    /// Check if a given L2 block is marked as finalized in this update.
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

/// Represents a group of duties created from a single sync event.
#[derive(Clone, Debug)]
pub struct DutyBatch {
    sync_ev_idx: u64,
    duties: Vec<DutyEntry>,
}

impl DutyBatch {
    /// Create a new duty batch for a single sync event.
    pub fn new(sync_ev_idx: u64, duties: Vec<DutyEntry>) -> Self {
        Self {
            sync_ev_idx,
            duties,
        }
    }

    /// Returns sync event idx that this duty batch was created from.
    pub fn sync_ev_idx(&self) -> u64 {
        self.sync_ev_idx
    }

    /// Returns reference to duties in this batch.
    pub fn duties(&self) -> &[DutyEntry] {
        &self.duties
    }
}

/// Sequencer key used for signing-related duties.
#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub enum IdentityKey {
    /// Sequencer private key used for signing.
    Sequencer(Buf32),
}

/// Container for signing identity key and verification identity key.
///
/// This is really just a stub that we should replace
/// with real cryptographic signatures and putting keys in the rollup params.
#[derive(Clone, Debug)]
pub struct IdentityData {
    /// Unique identifying info.
    pub ident: Identity,
    /// Signing key.
    pub key: IdentityKey,
}

impl IdentityData {
    /// Create new IdentityData from components.
    pub fn new(ident: Identity, key: IdentityKey) -> Self {
        Self { ident, key }
    }
}
