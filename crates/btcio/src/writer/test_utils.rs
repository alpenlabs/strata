use std::sync::Arc;

use bitcoin::{Address, Network};
use strata_db::{traits::BroadcastDatabase, types::L1TxEntry};
use strata_rocksdb::{
    broadcaster::db::BroadcastDb, sequencer::db::SequencerDB, test_utils::get_rocksdb_tmp_instance,
    L1BroadcastDb, RBSeqBlobDb,
};
use strata_storage::ops::{
    inscription::{Context, InscriptionDataOps},
    l1tx_broadcast::Context as BContext,
};

use crate::{
    broadcaster::L1BroadcastHandle,
    writer::config::{InscriptionFeePolicy, WriterConfig},
};

/// Returns `Arc` of `SequencerDB` for testing
pub fn get_db() -> Arc<SequencerDB<RBSeqBlobDb>> {
    let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
    let seqdb = Arc::new(RBSeqBlobDb::new(db, db_ops));
    Arc::new(SequencerDB::new(seqdb))
}

/// Returns `Arc` of `InscriptionDataOps` for testing
pub fn get_inscription_ops() -> Arc<InscriptionDataOps> {
    let pool = threadpool::Builder::new().num_threads(2).build();
    let db = get_db();
    let ops = Context::new(db).into_ops(pool);
    Arc::new(ops)
}

/// Returns `Arc` of `BroadcastDatabase` for testing
pub fn get_broadcast_db() -> Arc<impl BroadcastDatabase> {
    let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
    let bcastdb = Arc::new(L1BroadcastDb::new(db, dbops));
    Arc::new(BroadcastDb::new(bcastdb))
}

/// Returns `Arc` of `L1BroadcastHandle` for testing
pub fn get_broadcast_handle() -> Arc<L1BroadcastHandle> {
    let pool = threadpool::Builder::new().num_threads(2).build();
    let db = get_broadcast_db();
    let ops = BContext::new(db.l1_broadcast_db().clone()).into_ops(pool);
    let (sender, _) = tokio::sync::mpsc::channel::<(u64, L1TxEntry)>(64);
    let handle = L1BroadcastHandle::new(sender, Arc::new(ops));
    Arc::new(handle)
}

/// Returns an instance of [`WriterConfig`] with sensible defaults for testing
pub fn get_config() -> WriterConfig {
    let addr = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5"
        .parse::<Address<_>>()
        .unwrap()
        .require_network(Network::Regtest)
        .unwrap();
    WriterConfig {
        sequencer_address: addr,
        rollup_name: "strata".to_string(),
        inscription_fee_policy: InscriptionFeePolicy::Fixed(100),
        poll_duration_ms: 1000,
        amount_for_reveal_txn: 1000,
    }
}
