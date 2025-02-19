//! Sequencer duties.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::{buf::Buf32, hash::compute_borsh_hash, l2::L2BlockCommitment};
use strata_state::{batch::Checkpoint, id::L2BlockId};

/// Describes when we'll stop working to fulfill a duty.
#[derive(Clone, Debug)]
pub enum Expiry {
    /// Duty expires when we see the next block.
    NextBlock,

    /// Duty expires when block is finalized to L1 in a batch.
    BlockFinalized,

    /// Duty expires after a certain timestamp.
    Timestamp(u64),

    /// Duty expires after a specific L2 block is finalized
    BlockIdFinalized(L2BlockId),

    /// Duty expires after a specific checkpoint is finalized on bitcoin
    CheckpointIdxFinalized(u64),
}

/// Unique identifier for a duty.
pub type DutyId = Buf32;

/// Duties the sequencer might carry out.
#[derive(Clone, Debug, BorshSerialize, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Duty {
    /// Goal to sign a block.
    SignBlock(BlockSigningDuty),

    /// Goal to build and commit a batch.
    CommitBatch(CheckpointDuty),
}

impl Duty {
    /// Returns when the duty should expire.
    pub fn expiry(&self) -> Expiry {
        match self {
            Self::SignBlock(_) => Expiry::NextBlock,
            Self::CommitBatch(duty) => Expiry::CheckpointIdxFinalized(duty.0.batch_info().epoch()),
        }
    }

    /// Returns a unique identifier for the duty.
    pub fn generate_id(&self) -> Buf32 {
        match self {
            // We want Batch commitment duty to be unique by the checkpoint idx
            Self::CommitBatch(duty) => compute_borsh_hash(&duty.0.batch_info().epoch()),
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
pub struct CheckpointDuty(Checkpoint);

impl CheckpointDuty {
    /// Creates a new `CheckpointDuty` from a [`Checkpoint`].
    pub fn new(batch_checkpoint: Checkpoint) -> Self {
        Self(batch_checkpoint)
    }

    /// Consumes `self`, returning the inner [`Checkpoint`].
    pub fn into_inner(self) -> Checkpoint {
        self.0
    }

    /// Returns a reference to the inner [`Checkpoint`].
    pub fn inner(&self) -> &Checkpoint {
        &self.0
    }
}

/// Represents a single duty inside duty tracker.
#[derive(Clone, Debug)]
pub struct DutyEntry {
    /// ID used to help avoid re-performing a duty.
    id: Buf32,

    /// Duty data itself.
    duty: Duty,

    /// Block ID it was created for.  This could be used to cancel the duty if
    /// the block is reorged.
    source_block: L2BlockCommitment,
}

impl DutyEntry {
    pub fn new(id: Buf32, duty: Duty, source_block: L2BlockCommitment) -> Self {
        Self {
            id,
            duty,
            source_block,
        }
    }

    /// Get duty ID.
    pub fn id(&self) -> Buf32 {
        self.id
    }

    /// Get reference to Duty.
    pub fn duty(&self) -> &Duty {
        &self.duty
    }

    /// Gets the block commitment that duty was created from.
    pub fn source_block(&self) -> &L2BlockCommitment {
        &self.source_block
    }
}

/// Describes an update to the world state which we use to expire some duties.
#[derive(Clone, Debug)]
pub struct StateUpdate {
    /// The slot we're currently at.
    last_block_slot: u64,

    /// The current timestamp of the update.
    timestamp: u64,

    /// Newly finalized blocks, must be sorted.
    newly_finalized_blocks: Vec<L2BlockId>,

    /// Latest finalized block.
    latest_finalized_block: Option<L2BlockId>,

    /// Latest finalized batch.
    latest_finalized_batch: Option<u64>,
}

impl StateUpdate {
    /// Create a new state update.  The list of newly finalized blocks MUST be
    /// in reverse order, with the newest first.
    pub fn new(
        last_block_slot: u64,
        timestamp: u64,
        mut newly_finalized_blocks: Vec<L2BlockId>,
        latest_finalized_batch: Option<u64>,
    ) -> Self {
        // Extract latest finalized block before sorting
        let latest_finalized_block = newly_finalized_blocks.first().cloned();

        // Sort them so we can binary search afterwards.
        newly_finalized_blocks.sort();

        Self {
            last_block_slot,
            timestamp,
            newly_finalized_blocks,
            latest_finalized_block,
            latest_finalized_batch,
        }
    }

    /// Create state update without blocks or batch info.
    pub fn new_simple(last_block_slot: u64, cur_timestamp: u64) -> Self {
        Self::new(last_block_slot, cur_timestamp, Vec::new(), None)
    }

    /// Gets the last block slot.
    pub fn last_block_slot(&self) -> u64 {
        self.last_block_slot
    }

    /// Gets the timestamp of this update.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Gets the newly finalized blocks.
    pub fn newly_finalized_blocks(&self) -> &[L2BlockId] {
        &self.newly_finalized_blocks
    }

    /// Gets the latest finalized block, if there is one.
    pub fn latest_finalized_block(&self) -> Option<&L2BlockId> {
        self.latest_finalized_block.as_ref()
    }

    /// Gets the index of the latest finalized batch, if there is one.  This
    /// corresponds to the latest finalized epoch.
    pub fn latest_finalized_batch(&self) -> Option<u64> {
        self.latest_finalized_batch
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
    tip: L2BlockCommitment,
    duties: Vec<DutyEntry>,
}

impl DutyBatch {
    /// Create a new duty batch generated from a chain tip update.
    pub fn new(tip: L2BlockCommitment, duties: Vec<DutyEntry>) -> Self {
        Self { tip, duties }
    }

    /// Returns the chain tip this batch was derived from.
    pub fn tip(&self) -> &L2BlockCommitment {
        &self.tip
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
