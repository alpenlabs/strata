use std::sync::Arc;

use alpen_express_primitives::buf::Buf32;
use anyhow::Context;
use bitcoin::hashes::Hash;
use bitcoin::{consensus::serialize, Transaction};
use sha2::{Digest, Sha256};

use alpen_express_db::types::BlobL1Status;
use alpen_express_db::{
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::BlobEntry,
};

use crate::rpc::traits::BitcoinClient;

use super::builder::build_inscription_txs;
use super::config::WriterConfig;

// Helper function to fetch a blob entry from within tokio
pub async fn get_blob_by_idx<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    idx: u64,
) -> anyhow::Result<Option<BlobEntry>> {
    tokio::task::spawn_blocking(move || Ok(db.sequencer_provider().get_blob_by_idx(idx)?)).await?
}

// Helper function to fetch a blob entry from within tokio
pub async fn get_blob_by_id<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    id: Buf32,
) -> anyhow::Result<Option<BlobEntry>> {
    tokio::task::spawn_blocking(move || Ok(db.sequencer_provider().get_blob_by_id(id)?)).await?
}

// Helper to put blob from within tokio's context
pub async fn put_blob<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    id: Buf32,
    entry: BlobEntry,
) -> anyhow::Result<u64> {
    tokio::task::spawn_blocking(move || Ok(db.sequencer_store().put_blob(id, entry)?)).await?
}

// Helper function to update a blob entry by index from within tokio
pub async fn update_blob_by_idx<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    idx: u64,
    blob_entry: BlobEntry,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        Ok(db.sequencer_store().update_blob_by_idx(idx, blob_entry)?)
    })
    .await?
}

// Helper function to fetch a l1tx from within tokio
pub async fn get_l1_tx<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    txid: Buf32,
) -> anyhow::Result<Option<Vec<u8>>> {
    tokio::task::spawn_blocking(move || Ok(db.sequencer_provider().get_l1_tx(txid)?)).await?
}

// Helper function to store commit reveal txs
pub async fn put_commit_reveal_txs<D: SequencerDatabase + Send + Sync + 'static>(
    db: Arc<D>,
    commit_tx: Transaction,
    reveal_tx: Transaction,
) -> anyhow::Result<(Buf32, Buf32)> {
    let cid: Buf32 = commit_tx
        .compute_txid()
        .as_raw_hash()
        .to_byte_array()
        .into();
    let rid: Buf32 = reveal_tx
        .compute_txid()
        .as_raw_hash()
        .to_byte_array()
        .into();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        Ok(db.sequencer_store().put_commit_reveal_txs(
            cid,
            serialize(&commit_tx),
            rid,
            serialize(&reveal_tx),
        )?)
    })
    .await??;
    Ok((cid, rid))
}

pub async fn sign_transaction(
    client: &impl BitcoinClient,
    tx: Transaction,
) -> anyhow::Result<Transaction> {
    let tx = client.sign_raw_transaction_with_wallet(tx).await?;
    Ok(tx)
}

/// Type alias for blob index.
pub type BlobIdx = u64;

/// This will create inscription transactions corresponding to a blobidx and appropriately update
/// the blob. This is called when we receive a new intent as well as when broadcasting fails because
/// the input utxos have been spent by something else already
pub async fn create_and_sign_blob_inscriptions<D: SequencerDatabase + Send + Sync + 'static>(
    blobidx: BlobIdx,
    db: Arc<D>,
    client: Arc<impl BitcoinClient>,
    config: &WriterConfig,
) -> anyhow::Result<()> {
    if let Some(mut entry) = get_blob_by_idx(db.clone(), blobidx).await? {
        // TODO: handle insufficient utxos
        let (commit, reveal) = build_inscription_txs(&entry.blob, &client, config).await?;

        let signed_commit: Transaction = sign_transaction(client.as_ref(), commit)
            .await
            .context(format!("Signing commit tx failed for blob {}", blobidx))?;

        // We don't need to explicitly sign the reveal txn because we'll be doing key path spending
        // using the ephemeral key generated while building the inscriptions
        let (cid, rid) = put_commit_reveal_txs(db.clone(), signed_commit, reveal).await?;

        // Update the corresponding commit/reveal txids in entry along with the status from
        // `Unsigned`/`NeedResign` to `Unpublished`
        entry.commit_txid = cid;
        entry.reveal_txid = rid;
        entry.status = BlobL1Status::Unpublished;

        update_blob_by_idx(db, blobidx, entry).await?;
    }
    Ok(())
}

pub fn calculate_blob_hash(blob: &[u8]) -> Buf32 {
    let hash: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(blob);
        hasher.finalize().into()
    };
    Buf32(hash.into())
}

#[cfg(test)]
mod test {
    use std::{str::FromStr, sync::Arc};

    use bitcoin::{Address, Network};

    use alpen_express_db::traits::SequencerDatabase;
    use alpen_express_rocksdb::{
        sequencer::db::SequencerDB, test_utils::get_rocksdb_tmp_instance, SeqDb,
    };

    use super::*;
    use crate::writer::{
        config::{InscriptionFeePolicy, WriterConfig},
        test_utils::BitcoinDTestClient,
    };

    fn get_db() -> Arc<SequencerDB<SeqDb>> {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let seqdb = Arc::new(SeqDb::new(db, db_ops));
        Arc::new(SequencerDB::new(seqdb))
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
        let db = get_db();
        let client = Arc::new(BitcoinDTestClient::new(1));
        let config = get_config();

        // First insert an unsigned blob
        let entry = BlobEntry::new_unsigned([1; 100].to_vec());

        assert_eq!(entry.status, BlobL1Status::Unsigned);
        assert_eq!(entry.commit_txid, Buf32::zero());
        assert_eq!(entry.reveal_txid, Buf32::zero());

        let intent_hash = calculate_blob_hash(&entry.blob);
        let idx = db.sequencer_store().put_blob(intent_hash, entry).unwrap();

        create_and_sign_blob_inscriptions(idx, db.clone(), client, &config)
            .await
            .unwrap();

        // Now fetch blob entry
        let entry = db
            .sequencer_provider()
            .get_blob_by_idx(idx)
            .unwrap()
            .unwrap();
        assert_eq!(entry.status, BlobL1Status::Unpublished);

        assert!(entry.commit_txid != Buf32::zero());
        assert!(entry.reveal_txid != Buf32::zero());

        // Check if corresponding txs exist in db
        let ctx = db
            .sequencer_provider()
            .get_l1_tx(entry.commit_txid)
            .unwrap();
        assert!(ctx.is_some());
        let rtx = db
            .sequencer_provider()
            .get_l1_tx(entry.reveal_txid)
            .unwrap();
        assert!(rtx.is_some());
    }
}
