use bitcoin::{consensus::serialize, hashes::Hash, Block};
use strata_l1tx::messages::{BlockData, L1Event};
use strata_primitives::{
    batch::{verify_signed_checkpoint_sig, Checkpoint, CommitmentInfo, L1CommittedCheckpoint},
    buf::Buf32,
    l1::{
        generate_l1_tx, HeaderVerificationState, L1BlockCommitment, L1BlockManifest,
        L1HeaderRecord, L1Tx, ProtocolOperation,
    },
    params::RollupParams,
};
use strata_state::sync_event::{EventSubmitter, SyncEvent};
use tracing::*;

use super::query::ReaderContext;
use crate::rpc::traits::ReaderRpc;

pub(crate) async fn handle_bitcoin_event<R: ReaderRpc>(
    event: L1Event,
    ctx: &ReaderContext<R>,
    event_submitter: &impl EventSubmitter,
) -> anyhow::Result<()> {
    let sync_evs = match event {
        L1Event::RevertTo(block) => {
            // L1 reorgs will be handled in L2 STF, we just have to reflect
            // what the client is telling us in the database.
            let height = block.height();
            ctx.l1_manager.revert_canonical_chain_async(height).await?;
            debug!(%height, "reverted L1 block database");
            vec![SyncEvent::L1Revert(block)]
        }

        L1Event::BlockData(blockdata, epoch, hvs) => {
            handle_blockdata(ctx, blockdata, hvs, epoch).await?
        }
    };

    // Write to sync event db.
    for ev in sync_evs {
        event_submitter.submit_event_async(ev).await?;
    }
    Ok(())
}

async fn handle_blockdata<R: ReaderRpc>(
    ctx: &ReaderContext<R>,
    blockdata: BlockData,
    hvs: HeaderVerificationState,
    epoch: u64,
) -> anyhow::Result<Vec<SyncEvent>> {
    let ReaderContext {
        params, l1_manager, ..
    } = ctx;

    let height = blockdata.block_num();
    let mut sync_evs = Vec::new();

    // Bail out fast if we don't have to care.
    let horizon = params.rollup().horizon_l1_height;
    if height < horizon {
        warn!(%height, %horizon, "ignoring BlockData for block before horizon");
        return Ok(sync_evs);
    }

    let txs: Vec<_> = generate_l1txs(&blockdata);
    let num_txs = txs.len();
    let manifest = generate_block_manifest(blockdata.block(), hvs, txs.clone(), epoch, height);
    let l1blockid = *manifest.blkid();

    l1_manager.put_block_data_async(manifest, txs).await?;
    l1_manager
        .add_to_canonical_chain_async(height, &l1blockid)
        .await?;
    info!(%height, %l1blockid, txs = %num_txs, "wrote L1 block manifest");

    // Create a sync event if it's something we care about.
    let blkid: Buf32 = blockdata.block().block_hash().into();
    let block_commitment = L1BlockCommitment::new(height, blkid.into());
    sync_evs.push(SyncEvent::L1Block(block_commitment));

    Ok(sync_evs)
}

/// Extracts checkpoints from the block.
fn find_checkpoints(blockdata: &BlockData, params: &RollupParams) -> Vec<L1CommittedCheckpoint> {
    blockdata
        .relevant_txs()
        .iter()
        .flat_map(|txref| {
            txref.contents().protocol_ops().iter().map(|x| match x {
                ProtocolOperation::Checkpoint(envelope) => Some((
                    envelope,
                    &blockdata.block().txdata[txref.index() as usize],
                    txref.index(),
                )),
                _ => None,
            })
        })
        .filter_map(|ckpt_data| {
            let (signed_checkpoint, tx, position) = ckpt_data?;
            if !verify_signed_checkpoint_sig(signed_checkpoint, params) {
                error!(
                    ?tx,
                    ?signed_checkpoint,
                    "signature verification failed on checkpoint"
                );
                return None;
            }

            let checkpoint: Checkpoint = signed_checkpoint.clone().into();

            let blockhash = (*blockdata.block().block_hash().as_byte_array()).into();
            let txid = (*tx.compute_txid().as_byte_array()).into();
            let wtxid = (*tx.compute_wtxid().as_byte_array()).into();
            let block_height = blockdata.block_num();
            let commitment_info =
                CommitmentInfo::new(blockhash, txid, wtxid, block_height, position);

            Some(L1CommittedCheckpoint::new(checkpoint, commitment_info))
        })
        .collect()
}

/// Given a block, generates a manifest of the parts we care about that we can
/// store in the database.
fn generate_block_manifest(
    block: &Block,
    hvs: HeaderVerificationState,
    txs: Vec<L1Tx>,
    epoch: u64,
    height: u64,
) -> L1BlockManifest {
    let blockid = block.block_hash().into();
    let root = block
        .witness_root()
        .map(|x| x.to_byte_array())
        .unwrap_or_default();
    let header = serialize(&block.header);

    let rec = L1HeaderRecord::new(blockid, header, Buf32::from(root));
    L1BlockManifest::new(rec, hvs, txs, epoch, height)
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
