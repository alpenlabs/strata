// TODO move this to btcio crate, maybe consolidating into a single task with
// the query task

use std::sync::Arc;

use bitcoin::{consensus::serialize, hashes::Hash, Block};
use secp256k1::XOnlyPublicKey;
use strata_l1tx::messages::{BlockData, L1Event};
use strata_primitives::{
    batch::{BatchCheckpoint, BatchCheckpointWithCommitment, CommitmentInfo},
    block_credential::CredRule,
    buf::Buf32,
    l1::{
        generate_l1_tx, L1BlockCommitment, L1BlockId, L1BlockManifest, L1BlockRecord, L1Tx,
        ProtocolOperation,
    },
    params::Params,
    prelude::*,
};
use strata_state::sync_event::SyncEvent;
use strata_storage::L1BlockManager;
use tokio::sync::mpsc;
use tracing::*;

use crate::csm::ctl::CsmController;

/// Consumes L1 events and reflects them in the database.
pub fn bitcoin_data_handler_task(
    l1man: Arc<L1BlockManager>,
    csm_ctl: Arc<CsmController>,
    mut event_rx: mpsc::Receiver<L1Event>,
    params: Arc<Params>,
) -> anyhow::Result<()> {
    // Parse the sequencer pubkey once here as this involves and FFI call that we don't want to be
    // calling per event although it can be generated from the params passed to the relevant event
    // handler.
    let seq_pubkey = match params.rollup.cred_rule {
        CredRule::Unchecked => None,
        CredRule::SchnorrKey(buf32) => Some(
            XOnlyPublicKey::try_from(buf32)
                .expect("the sequencer pubkey must be valid in the params"),
        ),
    };

    while let Some(event) = event_rx.blocking_recv() {
        if let Err(e) = handle_bitcoin_event(event, &l1man, csm_ctl.as_ref(), &params, seq_pubkey) {
            error!(err = %e, "failed to handle L1 event");
        }
    }

    info!("L1 event stream closed, store task exiting...");
    Ok(())
}

fn handle_bitcoin_event(
    event: L1Event,
    l1man: &L1BlockManager,
    csm_ctl: &CsmController,
    params: &Arc<Params>,
    seq_pubkey: Option<XOnlyPublicKey>,
) -> anyhow::Result<()> {
    match event {
        L1Event::RevertTo(block) => {
            // L1 reorgs will be handled in L2 STF, we just have to reflect
            // what the client is telling us in the database.
            l1man.revert_to_height(block.height())?;
            debug!(height = %block.height(), "wrote revert");

            // Write to sync event db.
            let ev = SyncEvent::L1Revert(block);
            csm_ctl.submit_event(ev)?;

            Ok(())
        }

        L1Event::BlockData(blockdata, epoch, hvs) => {
            let height = blockdata.block_num();

            // Bail out fast if we don't have to care.
            let horizon = params.rollup().horizon_l1_height;
            if height < horizon {
                warn!(%height, %horizon, "ignoring BlockData for block before horizon");
                return Ok(());
            }

            let l1blkid = blockdata.block().block_hash();

            // Extract all the parts we want.
            // TODO clean this up, the parts are kinda weird
            let l1txs: Vec<_> = generate_l1txs(&blockdata);
            let num_txs = l1txs.len();
            let manifest = generate_block_manifest(blockdata.block(), hvs, l1txs.clone(), epoch);

            l1man.put_block_data(blockdata.block_num(), manifest, l1txs.clone())?;
            info!(%height, %l1blkid, txs = %num_txs, "wrote L1 block manifest");

            // Write to sync event db if it's something we care about.
            let block = L1BlockCommitment::new(height, L1BlockId::from(l1blkid));
            let ev = SyncEvent::L1Block(block);
            csm_ctl.submit_event(ev)?;

            // TODO remove, we don't do this here anymore
            // Check for da batch and send event accordingly
            /*debug!(%height, "Checking for da batch");
            let checkpoints = check_for_da_batch(&blockdata, seq_pubkey);
            debug!(?checkpoints, "Received checkpoints");
            if !checkpoints.is_empty() {
                let ev = SyncEvent::L1DABatch(height, checkpoints);
                csm_ctl.submit_event(ev)?;
            }*/

            // TODO: Check for deposits and forced inclusions and emit appropriate events

            Ok(())
        }

        L1Event::GenesisVerificationState(height, header_verification_state) => {
            let last_blkid = L1BlockId::from(header_verification_state.last_verified_block_hash);
            let block = L1BlockCommitment::new(height, last_blkid);
            //let ev = SyncEvent::L1BlockGenesis(block, header_verification_state);
            //csm_ctl.submit_event(ev)?;
            Ok(())
        }
    }
}

/// Parses envelopes and checks for batch data in the transactions
fn check_for_da_batch(
    blockdata: &BlockData,
    seq_pubkey: Option<XOnlyPublicKey>,
) -> Vec<BatchCheckpointWithCommitment> {
    let protocol_ops_txs = blockdata.relevant_txs();

    let signed_checkpts = protocol_ops_txs.iter().flat_map(|tx| {
        tx.contents()
            .protocol_ops()
            .iter()
            .filter_map(|op| match op {
                ProtocolOperation::Checkpoint(envelope) => {
                    Some((envelope, &blockdata.block().txdata[tx.index() as usize]))
                }
                _ => None,
            })
    });

    let sig_verified_checkpoints = signed_checkpts.filter_map(|(signed_checkpoint, tx)| {
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
        let checkpoint: BatchCheckpoint = signed_checkpoint.clone().into();

        let blockhash = Buf32::from(*blockdata.block().block_hash().as_byte_array());
        let txid = Buf32::from(*tx.compute_txid().as_byte_array());
        let wtxid = Buf32::from(*tx.compute_wtxid().as_byte_array());
        let block_height = blockdata.block_num();
        let position = blockdata
            .block()
            .txdata
            .iter()
            .position(|x| x == tx)
            .unwrap() as u32;
        let commitment_info = CommitmentInfo::new(blockhash, txid, wtxid, block_height, position);

        Some(BatchCheckpointWithCommitment::new(
            checkpoint,
            commitment_info,
        ))
    });
    sig_verified_checkpoints.collect()
}

/// Given a block, generates a manifest of the parts we care about that we can
/// store in the database.
fn generate_block_manifest(
    block: &Block,
    hvs: HeaderVerificationState,
    txs: Vec<L1Tx>,
    epoch: u64,
) -> L1BlockManifest {
    let blockid = block.block_hash().into();
    let root = block
        .witness_root()
        .map(|x| x.to_byte_array())
        .unwrap_or_default();
    let header = serialize(&block.header);

    let mf = L1BlockRecord::new(blockid, header, Buf32::from(root));
    L1BlockManifest::new(mf, hvs, txs, epoch)
}

fn generate_l1txs(blockdata: &BlockData) -> Vec<L1Tx> {
    blockdata
        .relevant_txs()
        .iter()
        .map(|ops_txs| {
            generate_l1_tx(
                blockdata.block(),
                ops_txs.index(),
                ops_txs.contents().protocol_ops().to_vec(),
            )
        })
        .collect()
}
