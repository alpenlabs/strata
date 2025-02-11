use bitcoin::{consensus::serialize, hashes::Hash, Block};
use secp256k1::XOnlyPublicKey;
use strata_l1tx::messages::{BlockData, L1Event};
use strata_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1BlockRecord},
};
use strata_state::{
    batch::{Checkpoint, CommitmentInfo, L1CommittedCheckpoint},
    l1::{generate_l1_tx, L1Tx},
    sync_event::{EventSubmitter, SyncEvent},
    tx::ProtocolOperation,
};
use tracing::*;

use super::query::ReaderContext;
use crate::rpc::traits::ReaderRpc;

pub(crate) async fn handle_bitcoin_event<R: ReaderRpc, E: EventSubmitter>(
    event: L1Event,
    ctx: &ReaderContext<R>,
    event_submitter: &E,
) -> anyhow::Result<()> {
    let ReaderContext {
        seq_pubkey,
        params,
        l1_manager,
        ..
    } = ctx;
    match event {
        L1Event::RevertTo(revert_blk_num) => {
            // L1 reorgs will be handled in L2 STF, we just have to reflect
            // what the client is telling us in the database.
            l1_manager.revert_to_height_async(revert_blk_num).await?;
            debug!(%revert_blk_num, "wrote revert");

            // Write to sync event db.
            let ev = SyncEvent::L1Revert(revert_blk_num);
            event_submitter.submit_event_async(ev).await?;

            Ok(())
        }

        L1Event::BlockData(blockdata, epoch) => {
            let height = blockdata.block_num();

            // Bail out fast if we don't have to care.
            let horizon = params.rollup().horizon_l1_height;
            if height < horizon {
                warn!(%height, %horizon, "ignoring BlockData for block before horizon");
                return Ok(());
            }

            let l1blkid = blockdata.block().block_hash();

            let manifest = generate_block_manifest(blockdata.block(), epoch);
            let l1txs: Vec<_> = generate_l1txs(&blockdata);
            let num_txs = l1txs.len();
            l1_manager
                .put_block_data_async(blockdata.block_num(), manifest, l1txs.clone())
                .await?;
            info!(%height, %l1blkid, txs = %num_txs, "wrote L1 block manifest");

            // Write to sync event db if it's something we care about.
            let blkid: Buf32 = blockdata.block().block_hash().into();
            let ev = SyncEvent::L1Block(blockdata.block_num(), blkid.into());
            event_submitter.submit_event_async(ev).await?;

            // Check for da batch and send event accordingly
            debug!(?height, "Checking for da batch");
            let checkpoints = check_for_commitments(&blockdata, *seq_pubkey);
            debug!(?checkpoints, "Received checkpoints");
            if !checkpoints.is_empty() {
                let ev = SyncEvent::L1DABatch(height, checkpoints);
                event_submitter.submit_event_async(ev).await?;
            }

            // TODO: Check for deposits and forced inclusions and emit appropriate events

            Ok(())
        }

        L1Event::GenesisVerificationState(height, header_verification_state) => {
            let ev = SyncEvent::L1BlockGenesis(height, header_verification_state);
            event_submitter.submit_event_async(ev).await?;
            Ok(())
        }
    }
}

/// Parses envelopes and checks for batch data in the transactions
fn check_for_commitments(
    blockdata: &BlockData,
    seq_pubkey: Option<XOnlyPublicKey>,
) -> Vec<L1CommittedCheckpoint> {
    let relevant_txs = blockdata.relevant_txs();

    let signed_checkpts = relevant_txs.iter().flat_map(|txref| {
        txref.contents().protocol_ops().iter().map(|x| match x {
            ProtocolOperation::Checkpoint(envelope) => Some((
                envelope,
                &blockdata.block().txdata[txref.index() as usize],
                txref.index(),
            )),
            _ => None,
        })
    });

    let sig_verified_checkpoints = signed_checkpts.filter_map(|ckpt_data| {
        let (signed_checkpoint, tx, position) = ckpt_data?;
        if let Some(seq_pubkey) = seq_pubkey {
            if !signed_checkpoint.verify_sig(&seq_pubkey.into()) {
                error!(
                    ?tx,
                    ?signed_checkpoint,
                    "signature verification failed on checkpoint"
                );
                return None;
            }
        }
        let checkpoint: Checkpoint = signed_checkpoint.clone().into();

        let blockhash = (*blockdata.block().block_hash().as_byte_array()).into();
        let txid = (*tx.compute_txid().as_byte_array()).into();
        let wtxid = (*tx.compute_wtxid().as_byte_array()).into();
        let block_height = blockdata.block_num();
        let commitment_info = CommitmentInfo::new(blockhash, txid, wtxid, block_height, position);

        Some(L1CommittedCheckpoint::new(checkpoint, commitment_info))
    });
    sig_verified_checkpoints.collect()
}

/// Given a block, generates a manifest of the parts we care about that we can
/// store in the database.
fn generate_block_manifest(block: &Block, epoch: u64) -> L1BlockManifest {
    let blockid = block.block_hash().into();
    let root = block
        .witness_root()
        .map(|x| x.to_byte_array())
        .unwrap_or_default();
    let header = serialize(&block.header);

    let mf = L1BlockRecord::new(blockid, header, Buf32::from(root));
    L1BlockManifest::new(mf, epoch)
}

fn generate_l1txs(blockdata: &BlockData) -> Vec<L1Tx> {
    blockdata
        .relevant_txs()
        .iter()
        .map(|tx_entry| {
            generate_l1_tx(
                blockdata.block(),
                tx_entry.index(),
                tx_entry.contents().protocol_ops().to_vec(),
            )
        })
        .collect()
}
