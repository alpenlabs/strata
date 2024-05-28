//! Database abstraction layers, building on what Reth supports.

use l1::schemas::{L1BlockSchema, MmrSchema, TxnSchema};
use rockbound::{schema::ColumnFamilyName, Schema};

pub mod errors;
pub mod l1;
pub mod macros;
pub mod traits;

pub type DbResult<T> = anyhow::Result<T, crate::errors::DbError>;

const STORE_COLUMN_FAMILIES: &[ColumnFamilyName] = &[
    L1BlockSchema::COLUMN_FAMILY_NAME,
    TxnSchema::COLUMN_FAMILY_NAME,
    MmrSchema::COLUMN_FAMILY_NAME,
    // TODO: add col families for other store types
];
