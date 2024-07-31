#![feature(btree_extract_if)] // remove when we remove the stubs

<<<<<<< HEAD
=======
//! Database abstraction layers, building on what Reth supports.

use l2::schemas::{L2BlockHeightSchema, L2BlockSchema, L2BlockStatusSchema};
use rockbound::{schema::ColumnFamilyName, Schema};

use crate::client_state::schemas::{ClientStateSchema, ClientUpdateOutputSchema};
use crate::l1::schemas::{L1BlockSchema, MmrSchema, TxnSchema};
use crate::sync_event::schemas::SyncEventSchema;
use crate::{
    chain_state::schemas::{ChainStateSchema, WriteBatchSchema},
    sequencer::schemas::{
        SeqBIdRevTxnIdxSchema, SeqBlobIdSchema, SeqBlobSchema, SeqL1TxIdSchema, SeqL1TxnSchema,
    },
};

pub mod bridge;
pub mod chain_state;
pub mod client_state;
>>>>>>> fcc9f6d (refactor: move bridge-db to db)
pub mod database;
pub mod errors;
pub mod traits;
pub mod types;

#[cfg(feature = "stubs")]
pub mod stubs;

pub type DbResult<T> = anyhow::Result<T, errors::DbError>;
