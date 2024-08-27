use std::{str::FromStr, sync::Arc};

use alpen_express_db::{traits::TxBroadcastDatabase, types::L1TxEntry};
use alpen_express_rocksdb::{
    broadcaster::db::BroadcastDatabase, sequencer::db::SequencerDB,
    test_utils::get_rocksdb_tmp_instance, BroadcastDb, SeqDb,
};
use bitcoin::{Address, Network};
use express_storage::ops::{
    inscription::{Context, InscriptionDataOps},
    l1tx_broadcast::Context as BContext,
};

use crate::{
    broadcaster::L1BroadcastHandle,
    writer::config::{InscriptionFeePolicy, WriterConfig},
};

pub fn get_db() -> Arc<SequencerDB<SeqDb>> {
    let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
    let seqdb = Arc::new(SeqDb::new(db, db_ops));
    Arc::new(SequencerDB::new(seqdb))
}

pub fn get_insc_ops() -> Arc<InscriptionDataOps> {
    let pool = threadpool::Builder::new().num_threads(2).build();
    let db = get_db();
    let ops = Context::new(db).into_ops(pool);
    Arc::new(ops)
}

pub fn get_bcast_db() -> Arc<impl TxBroadcastDatabase> {
    let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
    let bcastdb = Arc::new(BroadcastDb::new(db, dbops));
    Arc::new(BroadcastDatabase::new(bcastdb))
}

pub fn get_bcast_handle() -> Arc<L1BroadcastHandle> {
    let pool = threadpool::Builder::new().num_threads(2).build();
    let db = get_bcast_db();
    let ops = BContext::new(db).into_ops(pool);
    let (sender, _) = tokio::sync::mpsc::channel::<(u64, L1TxEntry)>(64);
    let handle = L1BroadcastHandle::new(sender, Arc::new(ops));
    Arc::new(handle)
}

pub fn get_config() -> WriterConfig {
    let addr = Address::from_str("bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5")
        .unwrap()
        .require_network(Network::Regtest)
        .unwrap();
    WriterConfig {
        sequencer_address: addr,
        rollup_name: "alpen".to_string(),
        inscription_fee_policy: InscriptionFeePolicy::Fixed(100),
        poll_duration_ms: 1000,
        amount_for_reveal_txn: 1000,
    }
}
