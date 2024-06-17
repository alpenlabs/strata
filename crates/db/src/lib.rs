//! Database abstraction layers, building on what Reth supports.

use l2::schemas::L2BlockSchema;
use rockbound::{schema::ColumnFamilyName, Schema};

use crate::consensus_state::schemas::{ConsensusOutputSchema, ConsensusStateSchema};
use crate::l1::schemas::{L1BlockSchema, MmrSchema, TxnSchema};
use crate::sync_event::schemas::SyncEventSchema;

pub mod consensus_state;
pub mod database;
pub mod l1;
pub mod l2;
pub mod stubs;
pub mod sync_event;

pub mod errors;
pub mod macros;
pub mod traits;

pub type DbResult<T> = anyhow::Result<T, errors::DbError>;

pub const ROCKSDB_NAME: &str = "vertex";

pub const STORE_COLUMN_FAMILIES: &[ColumnFamilyName] = &[
    ConsensusOutputSchema::COLUMN_FAMILY_NAME,
    ConsensusStateSchema::COLUMN_FAMILY_NAME,
    L1BlockSchema::COLUMN_FAMILY_NAME,
    MmrSchema::COLUMN_FAMILY_NAME,
    SyncEventSchema::COLUMN_FAMILY_NAME,
    TxnSchema::COLUMN_FAMILY_NAME,
    L2BlockSchema::COLUMN_FAMILY_NAME
    // TODO add col families for other store types
];

// Re-exports
pub use consensus_state::db::ConsensusStateDb;
pub use l1::db::L1Db;
pub use sync_event::db::SyncEventDb;
