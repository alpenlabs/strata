pub mod bridge;
pub mod bridge_relay;
pub mod broadcaster;
pub mod chain_state;
pub mod checkpoint;
pub mod client_state;
pub mod l1;
pub mod l2;
pub mod prover;
pub mod sequencer;
pub mod sync_event;

pub mod macros;
mod sequence;
pub mod utils;

use anyhow::Context;
use strata_db::database::CommonDatabase;

#[cfg(feature = "test_utils")]
pub mod test_utils;

pub const ROCKSDB_NAME: &str = "strata-client";

pub const STORE_COLUMN_FAMILIES: &[ColumnFamilyName] = &[
    SequenceSchema::COLUMN_FAMILY_NAME,
    ChainstateSchema::COLUMN_FAMILY_NAME,
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
    // Bcast schemas
    BcastL1TxIdSchema::COLUMN_FAMILY_NAME,
    BcastL1TxSchema::COLUMN_FAMILY_NAME,
    // Bridge relay schemas
    BridgeMsgIdSchema::COLUMN_FAMILY_NAME,
    ScopeMsgIdSchema::COLUMN_FAMILY_NAME,
    // Bridge signature schemas
    BridgeTxStateTxidSchema::COLUMN_FAMILY_NAME,
    BridgeTxStateSchema::COLUMN_FAMILY_NAME,
    // Bridge duty schemas
    BridgeDutyTxidSchema::COLUMN_FAMILY_NAME,
    BridgeDutyStatusSchema::COLUMN_FAMILY_NAME,
    // Bridge duty checkpoint
    BridgeDutyCheckpointSchema::COLUMN_FAMILY_NAME,
    // Checkpoint schemas
    BatchCheckpointSchema::COLUMN_FAMILY_NAME,
    // TODO add col families for other store types
];

use std::{fs, path::Path, sync::Arc};

use bridge::schemas::{
    BridgeDutyCheckpointSchema, BridgeDutyStatusSchema, BridgeDutyTxidSchema, BridgeTxStateSchema,
    BridgeTxStateTxidSchema,
};
pub const PROVER_COLUMN_FAMILIES: &[ColumnFamilyName] = &[
    SequenceSchema::COLUMN_FAMILY_NAME,
    prover::schemas::ProofSchema::COLUMN_FAMILY_NAME,
    prover::schemas::ProofDepsSchema::COLUMN_FAMILY_NAME,
];

// Re-exports
pub use bridge_relay::db::BridgeMsgDb;
use bridge_relay::schemas::*;
pub use broadcaster::db::L1BroadcastDb;
use broadcaster::{
    db::BroadcastDb,
    schemas::{BcastL1TxIdSchema, BcastL1TxSchema},
};
pub use chain_state::db::ChainstateDb;
pub use checkpoint::db::RBCheckpointDB;
use checkpoint::schemas::BatchCheckpointSchema;
pub use client_state::db::ClientStateDb;
pub use l1::db::L1Db;
use l2::{
    db::L2Db,
    schemas::{L2BlockHeightSchema, L2BlockSchema, L2BlockStatusSchema},
};
use rockbound::{schema::ColumnFamilyName, Schema};
pub use sequencer::db::RBSeqBlobDb;
use sequencer::{
    db::SequencerDB,
    schemas::{SeqBlobIdSchema, SeqBlobSchema},
};
pub use sync_event::db::SyncEventDb;

use crate::{
    chain_state::schemas::{ChainstateSchema, WriteBatchSchema},
    client_state::schemas::{ClientStateSchema, ClientUpdateOutputSchema},
    l1::schemas::{L1BlockSchema, MmrSchema, TxnSchema},
    sequence::SequenceSchema,
    sync_event::schemas::SyncEventSchema,
};

/// database operations configuration
#[derive(Clone, Copy, Debug)]
pub struct DbOpsConfig {
    pub retry_count: u16,
}

impl DbOpsConfig {
    pub fn new(retry_count: u16) -> Self {
        Self { retry_count }
    }
}

// Opens rocksdb database instance from datadir
pub fn open_rocksdb_database(
    datadir: &Path,
    dbname: &'static str,
) -> anyhow::Result<Arc<rockbound::OptimisticTransactionDB>> {
    let mut database_dir = datadir.to_path_buf();
    database_dir.push("rocksdb");

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let mut opts = rockbound::rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = rockbound::OptimisticTransactionDB::open(
        &database_dir,
        dbname,
        STORE_COLUMN_FAMILIES.iter().map(|s| s.to_string()),
        &opts,
    )
    .context("opening database")?;

    Ok(Arc::new(rbdb))
}

pub type CommonDb =
    CommonDatabase<L1Db, L2Db, SyncEventDb, ClientStateDb, ChainstateDb, RBCheckpointDB>;

pub fn init_core_dbs(
    rbdb: Arc<rockbound::OptimisticTransactionDB>,
    ops_config: DbOpsConfig,
) -> Arc<CommonDb> {
    // Initialize databases.
    let l1_db: Arc<_> = L1Db::new(rbdb.clone(), ops_config).into();
    let l2_db: Arc<_> = L2Db::new(rbdb.clone(), ops_config).into();
    let sync_ev_db: Arc<_> = SyncEventDb::new(rbdb.clone(), ops_config).into();
    let clientstate_db: Arc<_> = ClientStateDb::new(rbdb.clone(), ops_config).into();
    let chainstate_db: Arc<_> = ChainstateDb::new(rbdb.clone(), ops_config).into();
    let checkpoint_db: Arc<_> = RBCheckpointDB::new(rbdb.clone(), ops_config).into();
    let database = CommonDatabase::new(
        l1_db,
        l2_db,
        sync_ev_db,
        clientstate_db,
        chainstate_db,
        checkpoint_db,
    );

    database.into()
}

pub fn init_broadcaster_database(
    rbdb: Arc<rockbound::OptimisticTransactionDB>,
    ops_config: DbOpsConfig,
) -> Arc<BroadcastDb> {
    let l1_broadcast_db = L1BroadcastDb::new(rbdb.clone(), ops_config);
    BroadcastDb::new(l1_broadcast_db.into()).into()
}

pub fn init_sequencer_database(
    rbdb: Arc<rockbound::OptimisticTransactionDB>,
    ops_config: DbOpsConfig,
) -> Arc<SequencerDB<RBSeqBlobDb>> {
    let seqdb = RBSeqBlobDb::new(rbdb, ops_config).into();
    SequencerDB::new(seqdb).into()
}
