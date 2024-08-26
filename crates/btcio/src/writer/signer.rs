use std::sync::Arc;

use alpen_express_db::types::{BlobEntry, L1TxEntry};
use alpen_express_primitives::buf::Buf32;
use bitcoin::Transaction;

use express_storage::BroadcastDbOps;

use crate::rpc::traits::{L1Client, SeqL1Client};

use super::{builder::build_inscription_txs, config::WriterConfig};

type BlobIdx = u64;

/// This will create inscription transactions corresponding to a blobidx and appropriately update
/// the blob. This is called when we receive a new intent as well as when broadcasting fails because
/// the input utxos have been spent by something else already
pub async fn create_and_sign_blob_inscriptions(
    blobentry: &BlobEntry,
    bops: &BroadcastDbOps,
    client: Arc<impl L1Client + SeqL1Client>,
    config: &WriterConfig,
) -> anyhow::Result<(Buf32, Buf32)> {
    // TODO: handle insufficient utxos
    let (commit, reveal) = build_inscription_txs(&blobentry.blob, &client, config).await?;

    let signed_commit: Transaction = client.sign_raw_transaction_with_wallet(commit).await?;

    let cid: Buf32 = signed_commit.compute_txid().into();
    let rid: Buf32 = reveal.compute_txid().into();

    let centry = L1TxEntry::from_tx(&signed_commit);
    let rentry = L1TxEntry::from_tx(&reveal);

    // TODO: put the commit-reveal pair atomically in db
    let _cidx = bops.insert_new_tx_entry_async(cid, centry).await?;
    let _ridx = bops.insert_new_tx_entry_async(rid, rentry).await?;
    Ok((cid, rid))
}

#[cfg(test)]
mod test {
    use std::{str::FromStr, sync::Arc};

    use alpen_express_db::types::BlobEntry;
    use alpen_express_rocksdb::broadcaster::db::BroadcastDatabase;
    use alpen_express_rocksdb::BroadcastDb;
    use bitcoin::{Address, Network};

    use alpen_express_db::traits::TxBroadcastDatabase;
    use alpen_express_rocksdb::{
        sequencer::db::SequencerDB, test_utils::get_rocksdb_tmp_instance, SeqDb,
    };
    use express_storage::ops::inscription::Context;
    use express_storage::ops::l1tx_broadcast::Context as BContext;

    use super::*;
    use crate::test_utils::TestBitcoinClient;
    use crate::writer::config::{InscriptionFeePolicy, WriterConfig};

    fn get_db() -> Arc<SequencerDB<SeqDb>> {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seqdb = Arc::new(SeqDb::new(db, db_ops));
        Arc::new(SequencerDB::new(seqdb))
    }

    fn get_insc_ops() -> Arc<InscriptionDataOps> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let ops = Context::new(db).into_ops(pool);
        Arc::new(ops)
    }

    fn get_bcast_db() -> Arc<impl TxBroadcastDatabase> {
        let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(BroadcastDb::new(db, dbops));
        Arc::new(BroadcastDatabase::new(bcastdb))
    }

    fn get_bcast_ops() -> Arc<BroadcastDbOps> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_bcast_db();
        let ops = BContext::new(db).into_ops(pool);
        Arc::new(ops)
    }

    fn get_config() -> WriterConfig {
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

    #[tokio::test]
    async fn test_create_and_sign_blob_inscriptions() {
        let iops = get_insc_ops();
        let bops = get_bcast_ops();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();

        // First insert an unsigned blob
        let entry = BlobEntry::new_unsigned([1; 100].to_vec());

        assert_eq!(entry.status, BlobL1Status::Unsigned);
        assert_eq!(entry.commit_txid, Buf32::zero());
        assert_eq!(entry.reveal_txid, Buf32::zero());

        let intent_hash = calculate_blob_hash(&entry.blob);
        let idx = iops
            .put_blob_entry_async(intent_hash, entry)
            .await
            .unwrap()
            .unwrap();

        create_and_sign_blob_inscriptions(idx, iops.as_ref(), bops.as_ref(), client, &config)
            .await
            .unwrap();

        // Now fetch blob entry
        let id = iops.get_blob_id_async(idx).await.unwrap().unwrap();
        let entry = iops.get_blob_entry_async(id).await.unwrap().unwrap();
        assert_eq!(entry.status, BlobL1Status::Unpublished);

        assert!(entry.commit_txid != Buf32::zero());
        assert!(entry.reveal_txid != Buf32::zero());

        // Check if corresponding txs exist in db
        let ctx = bops
            .get_tx_entry_by_id_async(entry.commit_txid)
            .await
            .unwrap();
        let rtx = bops
            .get_tx_entry_by_id_async(entry.reveal_txid)
            .await
            .unwrap();
        assert!(ctx.is_some());
        assert!(rtx.is_some());
    }
}
