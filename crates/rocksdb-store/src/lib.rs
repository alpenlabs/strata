pub mod bridge_relay;
pub mod broadcaster;
pub mod chain_state;
pub mod client_state;
pub mod l1;
pub mod l2;
pub mod sequencer;
pub mod sync_event;

pub mod macros;
pub mod utils;

#[cfg(feature = "test_utils")]
pub mod test_utils;

pub const ROCKSDB_NAME: &str = "express";

pub const STORE_COLUMN_FAMILIES: &[ColumnFamilyName] = &[
    ChainStateSchema::COLUMN_FAMILY_NAME,
    ClientUpdateOutputSchema::COLUMN_FAMILY_NAME,
    ClientStateSchema::COLUMN_FAMILY_NAME,
    L1BlockSchema::COLUMN_FAMILY_NAME,
    MmrSchema::COLUMN_FAMILY_NAME,
    SyncEventSchema::COLUMN_FAMILY_NAME,
    TxnSchema::COLUMN_FAMILY_NAME,
    L2BlockSchema::COLUMN_FAMILY_NAME,
    L2BlockStatusSchema::COLUMN_FAMILY_NAME,
    L2BlockHeightSchema::COLUMN_FAMILY_NAME,
    WriteBatchSchema::COLUMN_FAMILY_NAME,
    // Seqdb schemas
    SeqBlobIdSchema::COLUMN_FAMILY_NAME,
    SeqBlobSchema::COLUMN_FAMILY_NAME,
    BatchCommitmentSchema::COLUMN_FAMILY_NAME,
    // Bcast schemas
    BcastL1TxIdSchema::COLUMN_FAMILY_NAME,
    BcastL1TxSchema::COLUMN_FAMILY_NAME,
    // TODO add col families for other store types
];

// Re-exports
pub use broadcaster::db::BroadcastDb;
use broadcaster::schemas::{BcastL1TxIdSchema, BcastL1TxSchema};
pub use chain_state::db::ChainStateDb;
pub use client_state::db::ClientStateDb;
pub use l1::db::L1Db;
use l2::schemas::{L2BlockHeightSchema, L2BlockSchema, L2BlockStatusSchema};
use rockbound::{schema::ColumnFamilyName, Schema};
pub use sequencer::db::SeqDb;
use sequencer::schemas::{BatchCommitmentSchema, SeqBlobIdSchema, SeqBlobSchema};
pub use sync_event::db::SyncEventDb;

use crate::{
    chain_state::schemas::{ChainStateSchema, WriteBatchSchema},
    client_state::schemas::{ClientStateSchema, ClientUpdateOutputSchema},
    l1::schemas::{L1BlockSchema, MmrSchema, TxnSchema},
    sync_event::schemas::SyncEventSchema,
};

/// database operations configuration
#[derive(Clone, Copy, Debug)]
pub struct DbOpsConfig {
    pub retry_count: u16,
}
