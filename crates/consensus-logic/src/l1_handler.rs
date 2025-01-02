use std::{panic, sync::Arc};

use bitcoin::{
    consensus::serialize,
    hashes::{sha256d, Hash},
    Block, Wtxid,
};
use secp256k1::XOnlyPublicKey;
use strata_db::traits::{Database, L1Database};
use strata_primitives::{
    block_credential::CredRule,
    buf::Buf32,
    l1::{L1BlockManifest, L1BlockRecord, L1TxProof},
    params::{Params, RollupParams},
    proof::RollupVerifyingKey,
};
use strata_risc0_adapter;
use strata_sp1_adapter;
use strata_state::{
    batch::{BatchCheckpoint, CheckpointProofOutput},
    l1::L1Tx,
    sync_event::SyncEvent,
    tx::ProtocolOperation,
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
) -> Vec<BatchCheckpoint> {
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
        Some(checkpoint)
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
    let blockid = Buf32::from(block.block_hash().to_raw_hash().to_byte_array());
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
            extract_l1tx_from_block(
                blockdata.block(),
                ops_txs.index(),
                ops_txs.proto_op().clone(),
            )
        })
        .collect()
}

/// Generates an L1 transaction with proof for a given transaction index in a block.
///
/// # Parameters
/// - `block`: The block containing the transactions.
/// - `idx`: The index of the transaction within the block's transaction data.
/// - `txid_bytes`: computed txid of the Tx in [u8;32] form
/// - `proto_op`: Protocol operation data after parsing and gathering relevant tx
///
/// # Returns
/// - An `L1Tx` struct containing the proof and the serialized transaction.
///
/// # Panics
/// - If the `idx` is out of bounds for the block's transaction data.
fn extract_l1tx_from_block(block: &Block, idx: u32, proto_op: ProtocolOperation) -> L1Tx {
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

    L1Tx::new(proof, tx, proto_op)
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
        proof.push(item.into());

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
    (proof, curr_level[0].into())
}
