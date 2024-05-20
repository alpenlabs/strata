//! Trait definitions for low level database interfaces.  This borrows some of
//! its naming conventions from reth.

use alpen_vertex_mmr::CompactMmr;
use alpen_vertex_primitives::{l1::*, prelude::*};
use alpen_vertex_state::consensus::ConsensusState;
use alpen_vertex_state::sync_event::{SyncAction, SyncEvent};

use crate::errors::*;

/// Common database interface that we can parameterize worker tasks over if
/// parameterizing them over each individual trait gets cumbersome or if we need
/// to use behavior that crosses different interfaces.
pub trait Database {
    type L1Store: L1DataStore;
    type L1Prov: L1DataProvider;
    type SeStore: SyncEventStore;
    type SeProv: SyncEventProvider;
    type CsStore: ConsensusStateStore;
    type CsProv: ConsensusStateProvider;

    // TODO accessors as needed
}

/// Storage interface to control our view of L1 data.
pub trait L1DataStore {
    /// Atomically extends the chain with a new block, providing the manifest
    /// and a list of transactions we find interesting.  Returns error if
    /// provided out-of-order.
    fn put_block_data(&self, idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) -> DbResult<()>;

    /// Stores an MMR checkpoint so we have to query less far back.  If the
    /// provided height does not match the entries in the MMR, will return an
    /// error.
    fn put_mmr_checkpoint(&self, idx: u64, mmr: CompactMmr) -> DbResult<()>;

    /// Resets the L1 chain tip to the specified block index.  The provided
    /// index will be the new chain tip that we store.
    fn revert_to_height(&self, idx: u64) -> DbResult<()>;

    // TODO DA scraping storage
}

/// Provider interface to view L1 data.
pub trait L1DataProvider {
    /// Gets the current chain tip index.
    fn get_chain_tip(&self) -> DbResult<u64>;

    /// Gets the block manifest for a block index.
    fn get_block_manifest(&self, idx: u64) -> DbResult<L1BlockManifest>;

    /// Returns a half-open interval of block hashes, if we have all of them
    /// present.  Otherwise, returns error.
    fn get_blockid_range(&self, start_idx: u64, end_idx: u64) -> DbResult<Vec<Buf32>>;

    /// Gets the interesting txs we stored in a block.
    fn get_block_txs(&self, idx: u64) -> DbResult<Option<Vec<L1TxRef>>>;

    /// Gets the tx with proof given a tx ref, if present.
    fn get_tx(&self, tx_ref: L1TxRef) -> DbResult<Option<L1Tx>>;

    /// Gets the last MMR checkpoint we stored before the given block height.
    /// Up to the caller to advance the MMR the rest of the way to the desired
    /// state.
    fn get_last_mmr_to(&self, idx: u64) -> DbResult<Option<CompactMmr>>;

    // TODO DA queries
}

/// Describes an L1 block and associated data that we need to keep around.
#[derive(Clone, Debug)]
pub struct L1BlockManifest {
    /// Block hash/ID, kept here so we don't have to be aware of the hash function
    /// here.  This is what we use in the MMR.
    blockid: Buf32,

    /// Block header and whatever additional data we might want to query.
    header: Vec<u8>,

    /// Merkle root for the transactions in the block.  For Bitcoin, this is
    /// actually the witness transactions root, since we care about the witness
    /// data.
    txs_root: Buf32,
}

impl L1BlockManifest {
    // TODO accessors as needed
}

/// Store to write new sync events.
pub trait SyncEventStore {
    /// Atomically writes a new sync event, returning its index.
    fn write_sync_event(&self, ev: SyncEvent) -> DbResult<u64>;

    /// Atomically clears sync events in a range, defined as a half-open
    /// interval.  This should only be used for deeply buried events where we'll
    /// never need to look at them again.
    fn clear_sync_event(&self, start_idx: u64, end_idx: u64) -> DbResult<()>;
}

/// Provider to query sync events.  This does not provide notifications, that
/// should be handled at a higher level.
pub trait SyncEventProvider {
    /// Returns the sync event, if we have one at the given index.
    fn get_sync_event(&self, idx: u64) -> DbResult<Option<SyncEvent>>;

    /// Returns the unix timestamp that a sync event was inserted.
    fn get_event_timestamp(&self, idx: u64) -> DbResult<Option<u64>>;
}

/// Writes consensus states
pub trait ConsensusStateStore {
    /// Writes a new consensus output for a given input index.  These input
    /// indexes correspond to indexes in [``SyncEventStore``] and
    /// [``SyncEventProvider``].  Will error if `idx - 1` does not exist (unless
    /// `idx` is 0) or if trying to overwrite a state, as this is almost
    /// certainly a bug.
    fn write_consensus_output(&self, idx: u64, output: ConsensusOutput) -> DbResult<()>;
}

pub trait ConsensusStateProvider {
    /// Gets the output consensus state for some input index.
    fn get_consensus_state(&self, idx: u64) -> DbResult<Option<ConsensusOutput>>;

    /// Gets the actions output from a consensus state transition.
    fn get_consensus_actions(&self, idx: u64) -> DbResult<Option<Vec<SyncAction>>>;
}

/// Output of a consensus state transition.  Both the consensus state and sync
/// actions.
#[derive(Clone, Debug)]
pub struct ConsensusOutput {
    state: ConsensusState,
    actions: Vec<SyncAction>,
}

impl ConsensusOutput {
    // TODO accessors as needed
}
