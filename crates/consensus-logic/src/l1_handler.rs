use std::sync::Arc;

use bitcoin::{consensus::serialize, hashes::Hash, Block};
use secp256k1::XOnlyPublicKey;
use strata_db::traits::{Database, L1Database};
use strata_primitives::{
    block_credential::CredRule,
    buf::Buf32,
    l1::{L1BlockManifest, L1BlockRecord},
    params::{Params, RollupParams},
    proof::RollupVerifyingKey,
};
use strata_risc0_adapter;
use strata_sp1_adapter;
use strata_state::{
    batch::{
        BatchCheckpoint, BatchCheckpointWithCommitment, CheckpointProofOutput, CommitmentInfo,
    },
    l1::{generate_l1_tx, L1Tx},
    sync_event::SyncEvent,
};
use strata_tx_parser::messages::{BlockData, L1Event};
use strata_zkvm::{ProofReceipt, ZkVmError, ZkVmResult};
use tokio::sync::mpsc;
use tracing::*;

use crate::csm::ctl::CsmController;

/// Consumes L1 events and reflects them in the database.
pub fn bitcoin_data_handler_task<D: Database + Send + Sync + 'static>(
    l1db: Arc<D::L1DB>,
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
        if let Err(e) =
            handle_bitcoin_event(event, l1db.as_ref(), csm_ctl.as_ref(), &params, seq_pubkey)
        {
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
    seq_pubkey: Option<XOnlyPublicKey>,
) -> anyhow::Result<()>
where
    L1D: L1Database + Sync + Send + 'static,
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
            l1db.put_block_data(blockdata.block_num(), manifest, l1txs.clone())?;
            info!(%height, %l1blkid, txs = %num_txs, "wrote L1 block manifest");

            // Write to sync event db if it's something we care about.
            let blkid: Buf32 = blockdata.block().block_hash().into();
            let ev = SyncEvent::L1Block(blockdata.block_num(), blkid.into());
            csm_ctl.submit_event(ev)?;

            // Check for da batch and send event accordingly
            debug!(?height, "Checking for da batch");
            let checkpoints = check_for_da_batch(&blockdata, seq_pubkey);
            debug!(?checkpoints, "Received checkpoints");
            if !checkpoints.is_empty() {
                let ev = SyncEvent::L1DABatch(height, checkpoints);
                csm_ctl.submit_event(ev)?;
            }

            // TODO: Check for deposits and forced inclusions and emit appropriate events

            Ok(())
        }

        L1Event::GenesisVerificationState(height, header_verification_state) => {
            let ev = SyncEvent::L1BlockGenesis(height, header_verification_state);
            csm_ctl.submit_event(ev)?;
            Ok(())
        }
    }
}

/// Parses inscriptions and checks for batch data in the transactions
fn check_for_da_batch(
    blockdata: &BlockData,
    seq_pubkey: Option<XOnlyPublicKey>,
) -> Vec<BatchCheckpointWithCommitment> {
    let protocol_ops_txs = blockdata.protocol_ops_txs();

    let signed_checkpts = protocol_ops_txs
        .iter()
        .filter_map(|ops_txs| match ops_txs.proto_op() {
            strata_state::tx::ProtocolOperation::Checkpoint(inscription) => Some((
                inscription,
                &blockdata.block().txdata[ops_txs.index() as usize],
            )),
            _ => None,
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

/// Verify that the provided checkpoint proof is valid for the verifier key.
///
/// # Caution
///
/// If the checkpoint proof is empty, this function returns an `Ok(())`.
pub fn verify_proof(
    checkpoint: &BatchCheckpoint,
    proof_receipt: &ProofReceipt,
    rollup_params: &RollupParams,
) -> ZkVmResult<()> {
    let rollup_vk = rollup_params.rollup_vk;
    let checkpoint_idx = checkpoint.batch_info().idx();
    let proof = checkpoint.proof();
    info!(%checkpoint_idx, "verifying proof");

    // FIXME: we are accepting empty proofs for now (devnet) to reduce dependency on the prover
    // infra.
    if rollup_params.proof_publish_mode.allow_empty() && proof_receipt.is_empty() {
        warn!(%checkpoint_idx, "verifying empty proof as correct");
        return Ok(());
    }

    let expected_public_output = checkpoint.get_proof_output();
    let actual_public_output: CheckpointProofOutput =
        borsh::from_slice(proof_receipt.public_values().as_bytes())
            .map_err(|e| ZkVmError::OutputExtractionError { source: e.into() })?;
    if expected_public_output != actual_public_output {
        dbg!(actual_public_output, expected_public_output);
        return Err(ZkVmError::ProofVerificationError(
            "Public output mismatch during proof verification".to_string(),
        ));
    }
    let public_params_raw = proof_receipt.public_values().as_bytes();

    // NOTE/TODO: this should also verify that this checkpoint is based on top of some previous
    // checkpoint
    match rollup_vk {
        RollupVerifyingKey::Risc0VerifyingKey(vk) => {
            strata_risc0_adapter::verify_groth16(proof, vk.as_ref(), public_params_raw)
        }
        RollupVerifyingKey::SP1VerifyingKey(vk) => {
            strata_sp1_adapter::verify_groth16(proof, vk.as_ref(), public_params_raw)
        }
        // In Native Execution mode, we do not actually generate the proof to verify. Checking
        // public parameters is sufficient.
        RollupVerifyingKey::NativeVerifyingKey(_) => Ok(()),
    }
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
                ops_txs.proto_op().clone(),
            )
        })
        .collect()
}
