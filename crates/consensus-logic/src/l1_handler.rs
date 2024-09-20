use std::sync::Arc;

use alpen_express_btcio::{parser::inscription::parse_inscription_data, reader::messages::{BlockData, L1Event}};
use alpen_express_db::traits::{Database, L1DataStore};
use alpen_express_primitives::{
    buf::Buf32, l1::L1BlockManifest, params::Params, utils::generate_l1_tx,
};
use alpen_express_state::{
    batch::{BatchCheckpoint, SignedBatchCheckpoint},
    sync_event::SyncEvent,
};
use bitcoin::{consensus::serialize, hashes::Hash, Block};
use tokio::sync::mpsc;
use tracing::*;

use crate::ctl::CsmController;

/// Consumes L1 events and reflects them in the database.
pub fn bitcoin_data_handler_task<D: Database + Send + Sync + 'static>(
    l1db: Arc<D::L1Store>,
    csm_ctl: Arc<CsmController>,
    mut event_rx: mpsc::Receiver<L1Event>,
    params: Arc<Params>,
) -> anyhow::Result<()> {
    while let Some(event) = event_rx.blocking_recv() {
        if let Err(e) = handle_bitcoin_event(event, l1db.as_ref(), csm_ctl.as_ref(), &params) {
            error!(err = %e, "failed to handle L1 event");
        }
    }

    info!("L1 event stream closed, store task exiting...");
    Ok(())
}

fn handle_bitcoin_event<L1D>(
    event: L1Event,
    l1db: &L1D,
    csm_ctl: &CsmController,
    params: &Arc<Params>,
) -> anyhow::Result<()>
where
    L1D: L1DataStore + Sync + Send + 'static,
{
    match event {
        L1Event::RevertTo(revert_blk_num) => {
            // L1 reorgs will be handled in L2 STF, we just have to reflect
            // what the client is telling us in the database.
            l1db.revert_to_height(revert_blk_num)?;
            debug!(%revert_blk_num, "wrote revert");

            // Write to sync event db.
            let ev = SyncEvent::L1Revert(revert_blk_num);
            csm_ctl.submit_event(ev)?;

            Ok(())
        }

        L1Event::BlockData(blockdata) => {
            let height = blockdata.block_num();

            // Bail out fast if we don't have to care.
            let horizon = params.rollup().horizon_l1_height;
            if height < horizon {
                warn!(%height, %horizon, "ignoring BlockData for block before horizon");
                return Ok(());
            }

            let l1blkid = blockdata.block().block_hash();

            let manifest = generate_block_manifest(blockdata.block());
            let l1txs: Vec<_> = blockdata
                .relevant_tx()
                .iter()
                .map(|(idx, parsed_tx)| generate_l1_tx(*idx, parsed_tx.clone() , blockdata.block()))
                .collect();
            let num_txs = l1txs.len();
            l1db.put_block_data(blockdata.block_num(), manifest, l1txs.clone())?;
            info!(%height, %l1blkid, txs = %num_txs, "wrote L1 block manifest");

            // Write to sync event db if it's something we care about.
            let blkid: Buf32 = blockdata.block().block_hash().into();
            let ev = SyncEvent::L1Block(blockdata.block_num(), blkid.into());
            csm_ctl.submit_event(ev)?;

            // Check for da batch and send event accordingly
            let checkpoints = check_for_da_batch(&blockdata);
            if !checkpoints.is_empty() {
                let ev = SyncEvent::L1DABatch(height, checkpoints);
                csm_ctl.submit_event(ev)?;
            }

            // TODO: Check for deposits and forced inclusions and emit appropriate events

            Ok(())
        }
    }
}

/// Parses inscriptions and checks for batch data in the transactions
fn check_for_da_batch(blockdata: &BlockData) -> Vec<BatchCheckpoint> {
    let binding = blockdata
        .relevant_tx_idxs();

    let txs = binding
        .iter()
        .map(|&idx| &blockdata.block().txdata[idx as usize]);

    let inscriptions = txs.filter_map(|tx| {
        tx.input[0].witness.tapscript().and_then(|scr| {
                let script = scr.to_owned();
                parse_inscription_data(&script)
                .map_err(|e| {
                    let txid = tx.compute_txid();
                    warn!(%txid, err = %e, "invalid inscription inside transaction which is marked as relevant");
                    e
                })
                .ok()
                .map(|x| (x, tx))
        })
    });
    let signed_checkpoints = inscriptions.filter_map(|(insc, tx)| {
        match borsh::from_slice::<SignedBatchCheckpoint>(insc.batch_data()) {
            Err(e) => {
                let txid = tx.compute_txid();
                warn!(%txid, err = %e, "could not deserialize blob inside inscription");
                None
            }
            Ok(v) => Some(v),
        }
    });

    // NOTE/TODO: this is where we would verify the checkpoint, i.e, verify block ranges, verify
    // proof, and whatever else that's necessary

    signed_checkpoints.map(Into::into).collect()
}

/// Given a block, generates a manifest of the parts we care about that we can
/// store in the database.
fn generate_block_manifest(block: &Block) -> L1BlockManifest {
    let blockid = Buf32::from(block.block_hash().to_raw_hash().to_byte_array());
    let root = block
        .witness_root()
        .map(|x| x.to_byte_array())
        .unwrap_or_default();
    let header = serialize(&block.header);

    L1BlockManifest::new(blockid, header, Buf32::from(root))
}
