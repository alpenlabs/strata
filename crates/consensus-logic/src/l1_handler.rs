use std::sync::Arc;

use alpen_express_btcio::reader::messages::{BlockData, L1Event};
use alpen_express_db::traits::{Database, L1DataStore};
use alpen_express_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1Tx, L1TxProof},
    params::Params,
    tx::RelevantTxInfo,
};
use alpen_express_state::{
    batch::{BatchCheckpoint, SignedBatchCheckpoint},
    sync_event::SyncEvent,
};
use bitcoin::{
    consensus::serialize,
    hashes::{sha256d, Hash},
    Block, Wtxid,
};
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
                .protocol_ops_txs()
                .iter()
                .map(|ops_txs| {
                    extract_l1tx_from_block(
                        blockdata.block(),
                        ops_txs.index(),
                        ops_txs.relevant_tx_infos().clone(),
                    )
                })
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
    let protocol_ops_txs = blockdata.protocol_ops_txs();

    let inscriptions =
        protocol_ops_txs
            .iter()
            .filter_map(|ops_txs| match ops_txs.relevant_tx_infos() {
                alpen_express_primitives::tx::RelevantTxInfo::RollupInscription(inscription) => {
                    Some((
                        inscription,
                        &blockdata.block().txdata[ops_txs.index() as usize],
                    ))
                }
                _ => None,
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

/// Generates an L1 transaction with proof for a given transaction index in a block.
///
/// # Parameters
/// - `idx`: The index of the transaction within the block's transaction data.
/// - `relevant_info`: Relevant information gathered after parsing.
/// - `block`: The block containing the transactions.
///
/// # Returns
/// - An `L1Tx` struct containing the proof and the serialized transaction.
///
/// # Panics
/// - If the `idx` is out of bounds for the block's transaction data.
fn extract_l1tx_from_block(block: &Block, idx: u32, relevant_info: RelevantTxInfo) -> L1Tx {
    assert!(
        (idx as usize) < block.txdata.len(),
        "utils: tx idx out of range of block txs"
    );
    let tx = &block.txdata[idx as usize];

    // Get all witness ids for txs
    let wtxids = &block
        .txdata
        .iter()
        .enumerate()
        .map(|(i, x)| {
            if i == 0 {
                Wtxid::all_zeros() // Coinbase's wtxid is all zeros
            } else {
                x.compute_wtxid()
            }
        })
        .collect::<Vec<_>>();
    let (cohashes, _wtxroot) = get_cohashes_from_wtxids(wtxids, idx);

    let proof = L1TxProof::new(idx, cohashes);
    let tx = serialize(tx);

    L1Tx::new(proof, tx, relevant_info)
}

/// Generates cohashes for an wtxid in particular index with in given slice of wtxids.
///
/// # Parameters
/// - `wtxids`: The witness txids slice
/// - `index`: The index of the txn for which we want the cohashes
///
/// # Returns
/// - A tuple `(Vec<Buf32>, Buf32)` containing the cohashes and the merkle root
///
/// # Panics
/// - If the `index` is out of bounds for the `wtxids` length
fn get_cohashes_from_wtxids(wtxids: &[Wtxid], index: u32) -> (Vec<Buf32>, Buf32) {
    assert!(
        (index as usize) < wtxids.len(),
        "The transaction index should be within the txids length"
    );

    let mut curr_level: Vec<_> = wtxids
        .iter()
        .cloned()
        .map(|x| x.to_raw_hash().to_byte_array())
        .collect();
    let mut curr_index = index;
    let mut proof = Vec::new();

    while curr_level.len() > 1 {
        let len = curr_level.len();
        if len % 2 != 0 {
            curr_level.push(curr_level[len - 1]);
        }

        let proof_item_index = if curr_index % 2 == 0 {
            curr_index + 1
        } else {
            curr_index - 1
        };

        let item = curr_level[proof_item_index as usize];
        proof.push(Buf32(item.into()));

        // construct pairwise hash
        curr_level = curr_level
            .chunks(2)
            .map(|pair| {
                let [a, b] = pair else {
                    panic!("utils: cohash chunk should be a pair");
                };
                let mut arr = [0u8; 64];
                arr[..32].copy_from_slice(a);
                arr[32..].copy_from_slice(b);
                *sha256d::Hash::hash(&arr).as_byte_array()
            })
            .collect::<Vec<_>>();
        curr_index >>= 1;
    }
    (proof, Buf32(curr_level[0].into()))
}
