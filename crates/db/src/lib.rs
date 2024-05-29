//! Database abstraction layers, building on what Reth supports.

use rockbound::{schema::ColumnFamilyName, Schema};

use cs::schemas::{ConsensusOutputSchema, ConsensusStateSchema};
use l1::schemas::{L1BlockSchema, MmrSchema, TxnSchema};

pub mod errors;
pub mod macros;
pub mod traits;

pub mod cs;
pub mod l1;

pub type DbResult<T> = anyhow::Result<T, crate::errors::DbError>;

const STORE_COLUMN_FAMILIES: &[ColumnFamilyName] = &[
    ConsensusOutputSchema::COLUMN_FAMILY_NAME,
    ConsensusStateSchema::COLUMN_FAMILY_NAME,
    L1BlockSchema::COLUMN_FAMILY_NAME,
    MmrSchema::COLUMN_FAMILY_NAME,
    TxnSchema::COLUMN_FAMILY_NAME,
    // TODO: add col families for other store types
];
