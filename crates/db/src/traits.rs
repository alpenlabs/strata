//! Trait definitions for low level database interfaces.  This borrows some of
//! its naming conventions from reth.

use std::sync::Arc;

use alpen_express_state::block::L2BlockBundle;
use borsh::{BorshDeserialize, BorshSerialize};
#[cfg(feature = "mocks")]
use mockall::automock;

use alpen_express_mmr::CompactMmr;
use alpen_express_primitives::{l1::*, prelude::*};
use alpen_express_state::chain_state::ChainState;
use alpen_express_state::client_state::ClientState;
use alpen_express_state::operation::*;
use alpen_express_state::prelude::*;
use alpen_express_state::state_op::WriteBatch;
use alpen_express_state::sync_event::SyncEvent;

use crate::types::TxnStatusEntry;
use crate::DbResult;

/// Common database interface that we can parameterize worker tasks over if
/// parameterizing them over each individual trait gets cumbersome or if we need
/// to use behavior that crosses different interfaces.
#[cfg_attr(feature = "mocks", automock(
    type L1Store=MockL1DataStore; type L1Prov=MockL1DataProvider;
    type L2Store=MockL2DataStore; type L2Prov=MockL2DataProvider;
    type SeStore=MockSyncEventStore; type SeProv=MockSyncEventProvider;
    type CsStore=MockClientStateStore; type CsProv=MockClientStateProvider;
    type ChsStore=MockChainstateStore; type ChsProv=MockChainstateProvider;
))]
pub trait Database {
    type L1Store: L1DataStore;
    type L1Prov: L1DataProvider;
    type L2Store: L2DataStore;
    type L2Prov: L2DataProvider;
    type SeStore: SyncEventStore;
    type SeProv: SyncEventProvider;
    type CsStore: ClientStateStore;
    type CsProv: ClientStateProvider;
    type ChsStore: ChainstateStore;
    type ChsProv: ChainstateProvider;

    fn l1_store(&self) -> &Arc<Self::L1Store>;
    fn l1_provider(&self) -> &Arc<Self::L1Prov>;
    fn l2_store(&self) -> &Arc<Self::L2Store>;
    fn l2_provider(&self) -> &Arc<Self::L2Prov>;
    fn sync_event_store(&self) -> &Arc<Self::SeStore>;
    fn sync_event_provider(&self) -> &Arc<Self::SeProv>;
    fn client_state_store(&self) -> &Arc<Self::CsStore>;
    fn client_state_provider(&self) -> &Arc<Self::CsProv>;
    fn chainstate_store(&self) -> &Arc<Self::ChsStore>;
    fn chainstate_provider(&self) -> &Arc<Self::ChsProv>;
}

/// Storage interface to control our view of L1 data.
#[cfg_attr(feature = "mocks", automock)]
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
#[cfg_attr(feature = "mocks", automock)]
pub trait L1DataProvider {
    /// Gets the current chain tip index.
    fn get_chain_tip(&self) -> DbResult<Option<u64>>;

    /// Gets the block manifest for a block index.
    fn get_block_manifest(&self, idx: u64) -> DbResult<Option<L1BlockManifest>>;

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

/// Store to write new sync events.
#[cfg_attr(feature = "mocks", automock)]
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
#[cfg_attr(feature = "mocks", automock)]
pub trait SyncEventProvider {
    /// Returns the index of the most recently written sync event.
    fn get_last_idx(&self) -> DbResult<Option<u64>>;

    /// Gets the sync event with some index, if it exists.
    fn get_sync_event(&self, idx: u64) -> DbResult<Option<SyncEvent>>;

    /// Gets the unix millis timestamp that a sync event was inserted.
    fn get_event_timestamp(&self, idx: u64) -> DbResult<Option<u64>>;
}

/// Writes client state updates and checkpoints.
#[cfg_attr(feature = "mocks", automock)]
pub trait ClientStateStore {
    /// Writes a new consensus output for a given input index.  These input
    /// indexes correspond to indexes in [``SyncEventStore``] and
    /// [``SyncEventProvider``].  Will error if `idx - 1` does not exist (unless
    /// `idx` is 0) or if trying to overwrite a state, as this is almost
    /// certainly a bug.
    fn write_client_update_output(&self, idx: u64, output: ClientUpdateOutput) -> DbResult<()>;

    /// Writes a new consensus checkpoint that we can cheaply resume from.  Will
    /// error if trying to overwrite a state.
    fn write_client_state_checkpoint(&self, idx: u64, state: ClientState) -> DbResult<()>;
}

/// Provides client state writes and checkpoints.
#[cfg_attr(feature = "mocks", automock)]
pub trait ClientStateProvider {
    /// Gets the idx of the last written state.  Or returns error if a bootstrap
    /// state has not been written yet.
    fn get_last_write_idx(&self) -> DbResult<u64>;

    /// Gets the output client state writes for some input index.
    fn get_client_state_writes(&self, idx: u64) -> DbResult<Option<Vec<ClientStateWrite>>>;

    /// Gets the actions output from a client state transition.
    fn get_client_update_actions(&self, idx: u64) -> DbResult<Option<Vec<SyncAction>>>;

    /// Gets the last consensus checkpoint idx.
    fn get_last_checkpoint_idx(&self) -> DbResult<u64>;

    /// Gets the idx of the last checkpoint up to the given input idx.  This is
    /// the idx we should resume at when playing out consensus writes since the
    /// saved checkpoint, which may be the same as the given idx (if we didn't
    /// receive any sync events since the last checkpoint.
    fn get_prev_checkpoint_at(&self, idx: u64) -> DbResult<u64>;

    /// Gets a state checkpoint at a previously written index, if it exists.
    fn get_state_checkpoint(&self, idx: u64) -> DbResult<Option<ClientState>>;
}

/// L2 data store for CL blocks.  Does not store anything about what we think
/// the L2 chain tip is, that's controlled by the consensus state.
#[cfg_attr(feature = "mocks", automock)]
pub trait L2DataStore {
    /// Stores an L2 block, does not care about the block height of the L2
    /// block.  Also sets the block's status to "unchecked".
    fn put_block_data(&self, block: L2BlockBundle) -> DbResult<()>;

    /// Tries to delete an L2 block from the store, returning if it really
    /// existed or not.  This should only be used for blocks well before some
    /// buried L1 finalization horizon.
    fn del_block_data(&self, id: L2BlockId) -> DbResult<bool>;

    /// Sets the block's validity status.
    fn set_block_status(&self, id: L2BlockId, status: BlockStatus) -> DbResult<()>;
}

/// Data provider for L2 blocks.
#[cfg_attr(feature = "mocks", automock)]
pub trait L2DataProvider {
    /// Gets the L2 block by its ID, if we have it.
    fn get_block_data(&self, id: L2BlockId) -> DbResult<Option<L2BlockBundle>>;

    /// Gets the L2 block IDs that we have at some height, in case there's more
    /// than one on competing forks.
    // TODO do we even want to permit this as being a possible thing?
    fn get_blocks_at_height(&self, idx: u64) -> DbResult<Vec<L2BlockId>>;

    /// Gets the validity status of a block.
    fn get_block_status(&self, id: L2BlockId) -> DbResult<Option<BlockStatus>>;
}

/// Gets the status of a block.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, BorshSerialize, BorshDeserialize)]
pub enum BlockStatus {
    /// Block's validity hasn't been checked yet.
    Unchecked,

    /// Block is valid, although this doesn't mean it's in the canonical chain.
    Valid,

    /// Block is invalid, for no particular reason.  We'd have to look somewhere
    /// else for that.
    Invalid,
}

/// Write trait for the (consensus layer) chain state database.  For now we only
/// have a modestly sized "toplevel" chain state and no "large" state like the
/// EL does.  This trait is designed to permit a change to storing larger state
/// like that in the future without *too* much extra effort.  We decide new
/// states by providing the database with a generic "write batch" and offloading
/// the effort of deciding how to compute that write batch to the database impl.
#[cfg_attr(feature = "mocks", automock)]
pub trait ChainstateStore {
    /// Writes the genesis chainstate at index 0.
    fn write_genesis_state(&self, toplevel: &ChainState) -> DbResult<()>;

    /// Stores a write batch in the database, possibly computing that state
    /// under the hood from the writes.  Will not overwrite existing data,
    /// previous writes must be purged first in order to be replaced.
    fn write_state_update(&self, idx: u64, batch: &WriteBatch) -> DbResult<()>;

    /// Tells the database to purge state before a certain block index (height).
    fn purge_historical_state_before(&self, before_idx: u64) -> DbResult<()>;

    /// Rolls back any writes and state checkpoints after a specified block.
    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()>;
}

/// Read trait corresponding to [``ChainstateStore``].  See that trait's doc for
/// design explanation.
#[cfg_attr(feature = "mocks", automock)]
pub trait ChainstateProvider {
    /// Gets the last written state.
    fn get_last_state_idx(&self) -> DbResult<u64>;

    /// Gets the earliest written state.  This corresponds to calls to
    /// `purge_historical_state_before`.
    fn get_earliest_state_idx(&self) -> DbResult<u64>;

    /// Gets the write batch stored to compute a height.
    fn get_writes_at(&self, idx: u64) -> DbResult<Option<WriteBatch>>;

    /// Gets the toplevel chain state at a particular block index (height).
    fn get_toplevel_state(&self, idx: u64) -> DbResult<Option<ChainState>>;
}

#[cfg_attr(feature = "mocks", automock(type SeqStore=MockSeqDataStore; type SeqProv=MockSeqDataProvider;))]
pub trait SequencerDatabase {
    type SeqStore: SeqDataStore;
    type SeqProv: SeqDataProvider;

    fn sequencer_store(&self) -> &Arc<Self::SeqStore>;
    fn sequencer_provider(&self) -> &Arc<Self::SeqProv>;
}

#[cfg_attr(feature = "mocks", automock)]
pub trait SeqDataStore {
    /// Store the blob. Also create and store appropriate blob idx -> blobid mapping.
    /// Returns new blobidx, and returns error if entry already exists
    fn put_blob(&self, blob_id: Buf32, blob: Vec<u8>) -> DbResult<u64>;

    /// Store commit-reveal transactions, along with reveal txid -> blobid mapping, all of which
    /// should happen atomically.
    /// Returns the reveal txn idx
    fn put_commit_reveal_txns(
        &self,
        blobid: Buf32,
        commit_txn: TxnStatusEntry,
        reveal_txn: TxnStatusEntry,
    ) -> DbResult<u64>;

    /// Update an existing transaction
    fn update_txn(&self, txidx: u64, txn: TxnStatusEntry) -> DbResult<()>;
}

#[cfg_attr(feature = "mocks", automock)]
pub trait SeqDataProvider {
    /// Get the l1 inscription txn by idx
    fn get_l1_txn(&self, idx: u64) -> DbResult<Option<TxnStatusEntry>>;

    /// Get blob by its hash
    fn get_blob_by_id(&self, id: Buf32) -> DbResult<Option<Vec<u8>>>;

    /// Get the last blob idx
    fn get_last_blob_idx(&self) -> DbResult<Option<u64>>;

    /// Get the last txn idx
    fn get_last_txn_idx(&self) -> DbResult<Option<u64>>;

    ///Get the reveal tx idx associated with blob idx
    fn get_reveal_txidx_for_blob(&self, blobid: Buf32) -> DbResult<Option<u64>>;

    /// Get the blob id for blob idx
    fn get_blobid_for_blob_idx(&self, blobidx: u64) -> DbResult<Option<Buf32>>;
}
