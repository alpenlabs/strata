use std::sync::Arc;

use bitcoin::{consensus, Transaction};
use strata_btcio_rpc_types::traits::{Reader, Signer, Wallet};
use strata_btcio_tx::reveal::builder::{build_commit_reveal_txs, CommitRevealTxError};
use strata_db::types::{DataBundleIntentEntry, L1TxEntry};
use strata_primitives::buf::Buf32;
use tracing::*;

use super::config::WriterConfig;
use crate::broadcaster::L1BroadcastHandle;

/// Create commit reveal transactions corresponding to a [`EnvelopeEntry`].
///
/// This is used during one of the cases:
/// 1. A new blob intent needs to be signed
/// 2. A signed intent needs to be resigned because somehow its inputs were spent/missing
/// 3. A confirmed block that includes the tx gets reorged
pub async fn create_and_sign_commit_reveal_txs(
    blobentry: &DataBundleIntentEntry,
    broadcast_handle: &L1BroadcastHandle,
    client: Arc<impl Reader + Wallet + Signer>,
    config: &WriterConfig,
) -> Result<(Buf32, Buf32), CommitRevealTxError> {
    trace!("Creating and signing commit reveal transactions");
    let (commit, reveal) =
        build_commit_reveal_txs(&blobentry.envelopes, &client, &config.envelope_tx_config).await?;

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
        .map_err(|e| CommitRevealTxError::Other(e.into()))?;
    let _ = broadcast_handle
        .put_tx_entry(rid, rentry)
        .await
        .map_err(|e| CommitRevealTxError::Other(e.into()))?;
    Ok((cid, rid))
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use strata_db::types::{BundleL1Status, DataBundleIntentEntry};
    use strata_primitives::hash;
    use strata_state::{
        da_blob::PayloadCommitment,
        tx::{EnvelopePayload, PayloadTypeTag},
    };

    use super::*;
    use crate::{
        test_utils::TestBitcoinClient,
        writer::test_utils::{get_broadcast_handle, get_config, get_envelope_ops},
    };

    #[tokio::test]
    async fn test_create_and_sign_commit_reveal_txs() {
        let iops = get_envelope_ops();
        let bcast_handle = get_broadcast_handle();
        let client = Arc::new(TestBitcoinClient::new(1));
        let config = get_config();

        // First insert an unsigned blob
        let envelope = EnvelopePayload::new(PayloadTypeTag::DA, vec![1; 100]);
        let entry = DataBundleIntentEntry::new_unsigned(vec![envelope.clone()]);

        assert_eq!(entry.status, BundleL1Status::Unsigned);
        assert_eq!(entry.commit_txid, Buf32::zero());
        assert_eq!(entry.reveal_txid, Buf32::zero());

        let intent_hash = PayloadCommitment::new(&hash::raw(envelope.data()));
        // let intent_hash =
        iops.put_entry_async(intent_hash.into_inner(), entry.clone())
            .await
            .unwrap();

        let (cid, rid) =
            create_and_sign_commit_reveal_txs(&entry, bcast_handle.as_ref(), client, &config)
                .await
                .unwrap();

        // Check if corresponding txs exist in db
        let ctx = bcast_handle.get_tx_entry_by_id_async(cid).await.unwrap();
        let rtx = bcast_handle.get_tx_entry_by_id_async(rid).await.unwrap();
        assert!(ctx.is_some());
        assert!(rtx.is_some());
    }
}
