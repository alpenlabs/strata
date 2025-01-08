use std::sync::Arc;

use bitcoin::{consensus, Transaction};
use strata_db::types::{L1TxEntry, PayloadEntry};
use strata_primitives::buf::Buf32;
use tracing::*;

use super::{
    builder::{build_envelope_txs, EnvelopeError},
    context::WriterContext,
};
use crate::{
    broadcaster::L1BroadcastHandle,
    rpc::traits::{Reader, Signer, Wallet},
};

type BlobIdx = u64;

/// Create envelope transactions corresponding to a [`BlobEntry`].
///
/// This is used during one of the cases:
/// 1. A new payload intent needs to be signed
/// 2. A signed intent needs to be resigned because somehow its inputs were spent/missing
/// 3. A confirmed block that includes the tx gets reorged
pub async fn create_and_sign_payload_envelopes<T: Reader + Wallet + Signer>(
    payloadentry: &PayloadEntry,
    broadcast_handle: &L1BroadcastHandle,
    ctx: Arc<WriterContext<T>>,
) -> Result<(Buf32, Buf32), EnvelopeError> {
    trace!("Creating and signing payload envelopes");
    let (commit, reveal) = build_envelope_txs(&payloadentry.payload, ctx.as_ref()).await?;

    let ctxid = commit.compute_txid();
    debug!(commit_txid = ?ctxid, "Signing commit transaction");
    let signed_commit = ctx
        .client
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
        .map_err(|e| EnvelopeError::Other(e.into()))?;
    let _ = broadcast_handle
        .put_tx_entry(rid, rentry)
        .await
        .map_err(|e| EnvelopeError::Other(e.into()))?;
    Ok((cid, rid))
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use bitcoin::{Address, Network};
    use strata_config::btcio::BtcIOConfig;
    use strata_db::types::{PayloadEntry, PayloadL1Status};
    use strata_primitives::hash;
    use strata_state::da_blob::L1Payload;
    use strata_status::StatusChannel;
    use strata_test_utils::{l2::gen_params, ArbitraryGenerator};

    use super::*;
    use crate::{
        test_utils::TestBitcoinClient,
        writer::test_utils::{get_broadcast_handle, get_envelope_ops},
    };

    fn get_writer_context() -> Arc<WriterContext<TestBitcoinClient>> {
        let client = Arc::new(TestBitcoinClient::new(1));
        let addr = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5"
            .parse::<Address<_>>()
            .unwrap()
            .require_network(Network::Regtest)
            .unwrap();
        let cfg = Arc::new(BtcIOConfig::default());
        let status_channel = StatusChannel::new(
            ArbitraryGenerator::new().generate(),
            ArbitraryGenerator::new().generate(),
            None,
        );
        let params = Arc::new(gen_params());
        let ctx = WriterContext::new(params, cfg, addr, client, status_channel);
        Arc::new(ctx)
    }

    #[tokio::test]
    async fn test_create_and_sign_blob_envelopes() {
        let iops = get_envelope_ops();
        let bcast_handle = get_broadcast_handle();
        let ctx = get_writer_context();

        // First insert an unsigned blob
        let entry = PayloadEntry::new_unsigned(L1Payload::new_da([1; 100].to_vec()));

        assert_eq!(entry.status, PayloadL1Status::Unsigned);
        assert_eq!(entry.commit_txid, Buf32::zero());
        assert_eq!(entry.reveal_txid, Buf32::zero());

        let intent_hash = hash::raw(entry.payload.data());
        iops.put_payload_entry_async(intent_hash, entry.clone())
            .await
            .unwrap();

        let (cid, rid) = create_and_sign_payload_envelopes(&entry, bcast_handle.as_ref(), ctx)
            .await
            .unwrap();

        // Check if corresponding txs exist in db
        let ctx = bcast_handle.get_tx_entry_by_id_async(cid).await.unwrap();
        let rtx = bcast_handle.get_tx_entry_by_id_async(rid).await.unwrap();
        assert!(ctx.is_some());
        assert!(rtx.is_some());
    }
}
