use bitcoin::{consensus::serialize, hashes::Hash, Block};
use secp256k1::XOnlyPublicKey;
use strata_l1tx::messages::{BlockData, L1Event};
use strata_primitives::{
    buf::Buf32,
    l1::{L1BlockManifest, L1BlockRecord},
    params::Params,
};
use strata_state::{
    batch::{BatchCheckpoint, BatchCheckpointWithCommitment, CommitmentInfo},
    l1::{generate_l1_tx, L1Tx},
    sync_event::SyncEvent,
    tx::ProtocolOperation,
};
use strata_storage::L1BlockManager;
use tracing::*;

pub(crate) fn handle_bitcoin_event(
    event: L1Event,
    l1mgr: &L1BlockManager,
    submit_event: &impl Fn(SyncEvent) -> anyhow::Result<()>,
    params: &Params,
    seq_pubkey: &Option<XOnlyPublicKey>,
) -> anyhow::Result<()> {
    match event {
        L1Event::RevertTo(revert_blk_num) => {
            // L1 reorgs will be handled in L2 STF, we just have to reflect
            // what the client is telling us in the database.
            l1mgr.revert_to_height(revert_blk_num)?;
            debug!(%revert_blk_num, "wrote revert");

            // Write to sync event db.
            let ev = SyncEvent::L1Revert(revert_blk_num);
            submit_event(ev)?;

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
            l1mgr.put_block_data(blockdata.block_num(), manifest, l1txs.clone())?;
            info!(%height, %l1blkid, txs = %num_txs, "wrote L1 block manifest");

            // Write to sync event db if it's something we care about.
            let blkid: Buf32 = blockdata.block().block_hash().into();
            let ev = SyncEvent::L1Block(blockdata.block_num(), blkid.into());
            submit_event(ev)?;

            // Check for da batch and send event accordingly
            debug!(?height, "Checking for da batch");
            let checkpoints = check_for_da_batch(&blockdata, *seq_pubkey);
            debug!(?checkpoints, "Received checkpoints");
            if !checkpoints.is_empty() {
                let ev = SyncEvent::L1DABatch(height, checkpoints);
                submit_event(ev)?;
            }

            // TODO: Check for deposits and forced inclusions and emit appropriate events

            Ok(())
        }

        L1Event::GenesisVerificationState(height, header_verification_state) => {
            let ev = SyncEvent::L1BlockGenesis(height, header_verification_state);
            submit_event(ev)?;
            Ok(())
        }
    }
}

/// Parses envelopes and checks for batch data in the transactions
fn check_for_da_batch(
    blockdata: &BlockData,
    seq_pubkey: Option<XOnlyPublicKey>,
) -> Vec<BatchCheckpointWithCommitment> {
    let protocol_ops_txs = blockdata.protocol_ops_txs();

    let signed_checkpts = protocol_ops_txs.iter().flat_map(|txref| {
        txref.proto_ops().iter().filter_map(|op| match op {
            ProtocolOperation::Checkpoint(envelope) => {
                Some((envelope, &blockdata.block().txdata[txref.index() as usize]))
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
        .protocol_ops_txs()
        .iter()
        .map(|ops_txs| {
            generate_l1_tx(
                blockdata.block(),
                ops_txs.index(),
                ops_txs.proto_ops().to_vec(),
            )
        })
        .collect()
}
