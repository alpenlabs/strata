use std::sync::Arc;

use bitcoin::{consensus, Transaction};
use strata_db::types::{BlobEntry, L1TxEntry};
use strata_primitives::buf::Buf32;
use tracing::*;

use super::{
    builder::{build_inscription_txs, InscriptionError},
    config::WriterConfig,
};
use crate::{
    broadcaster::L1BroadcastHandle,
    rpc::traits::{Reader, Signer, Wallet},
};

type BlobIdx = u64;

/// Create inscription transactions corresponding to a [`BlobEntry`].
///
/// This is used during one of the cases:
/// 1. A new blob intent needs to be signed
/// 2. A signed intent needs to be resigned because somehow its inputs were spent/missing
/// 3. A confirmed block that includes the tx gets reorged
pub async fn create_and_sign_blob_inscriptions(
    blobentry: &BlobEntry,
    broadcast_handle: &L1BroadcastHandle,
    client: Arc<impl Reader + Wallet + Signer>,
    config: &WriterConfig,
) -> Result<(Buf32, Buf32), InscriptionError> {
    trace!("Creating and signing blob inscriptions");
    let (commit, reveal) = build_inscription_txs(&blobentry.blob, &client, config).await?;

    let ctxid = commit.compute_txid();
    debug!(commit_txid = ?ctxid, "Signing commit transaction");
    let signed_commit = client
        .sign_raw_transaction_with_wallet(&commit)
        .await
        .expect("could not sign commit tx")
        .hex;

    let signed_commit: Transaction = consensus::encode::deserialize_hex(&signed_commit)
        .expect("could not deserialize transaction");
    let cid: Buf32 = signed_commit.compute_txid().into();
    let rid: Buf32 = reveal.compute_txid().into();

    let centry = L1TxEntry::from_tx(&signed_commit);
    let rentry = L1TxEntry::from_tx(&reveal);

    // These don't need to be atomic. It will be handled by writer task if it does not find both
    // commit-reveal txs in db by triggering re-signing.
    let _ = broadcast_handle
        .put_tx_entry(cid, centry)
        .await
        .map_err(|e| InscriptionError::Other(e.into()))?;
    let _ = broadcast_handle
        .put_tx_entry(rid, rentry)
        .await
        .map_err(|e| InscriptionError::Other(e.into()))?;
    Ok((cid, rid))
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use strata_db::types::{BlobEntry, BlobL1Status};
    use strata_primitives::hash;

    use super::*;
    use crate::{
        test_utils::TestBitcoinClient,
        writer::test_utils::{get_broadcast_handle, get_config, get_inscription_ops},
    };

    #[tokio::test]
    async fn test_create_and_sign_blob_inscriptions() {
        let iops = get_inscription_ops();
        let bcast_handle = get_broadcast_handle();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();

        // First insert an unsigned blob
        let entry = BlobEntry::new_unsigned([1; 100].to_vec());

        assert_eq!(entry.status, BlobL1Status::Unsigned);
        assert_eq!(entry.commit_txid, Buf32::zero());
        assert_eq!(entry.reveal_txid, Buf32::zero());

        let intent_hash = hash::raw(&entry.blob);
        iops.put_blob_entry_async(intent_hash, entry.clone())
            .await
            .unwrap();

        let (cid, rid) =
            create_and_sign_blob_inscriptions(&entry, bcast_handle.as_ref(), client, &config)
                .await
                .unwrap();

        // Check if corresponding txs exist in db
        let ctx = bcast_handle.get_tx_entry_by_id_async(cid).await.unwrap();
        let rtx = bcast_handle.get_tx_entry_by_id_async(rid).await.unwrap();
        assert!(ctx.is_some());
        assert!(rtx.is_some());
    }
}
