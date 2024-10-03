//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::{
    batch::{BatchInfo, BootstrapState},
    id::L2BlockId,
    tx::DepositInfo,
};
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::L1BatchProofOutput;
use express_zkvm::Proof;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct ChainStateSnapshot {
    pub hash: Buf32,
    pub slot: u64,
    pub l2_blockid: L2BlockId,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct L2BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub initial_snapshot: ChainStateSnapshot,
    pub final_snapshot: ChainStateSnapshot,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    pub l1_state: L1BatchProofOutput,
    pub l2_state: L2BatchProofOutput,
    /// The verifying key of this checkpoint program.
    /// Required for verifying the Groth16 proof of this program.
    /// Cannot be hardcoded as any change to the program or proof implementation
    /// will change verifying_key.
    pub vk: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshDeserialize, BorshSerialize)]
pub struct CheckpointProofOutput {
    pub info: BatchInfo,
    pub bootstrap: BootstrapState,
    /// The verifying key of this checkpoint program.
    /// Required for verifying the Groth16 proof of this program.
    /// Cannot be hardcoded as any change to the program or proof implementation
    /// will change verifying_key.
    pub vk: Vec<u8>,
}

impl CheckpointProofOutput {
    pub fn new(info: BatchInfo, bootstrap: BootstrapState, vk: Vec<u8>) -> CheckpointProofOutput {
        Self {
            info,
            bootstrap,
            vk,
        }
    }
}

pub fn process_checkpoint_proof(
    l1_batch: &L1BatchProofOutput,
    l2_batch: &L2BatchProofOutput,
    vk: &[u8],
) -> (
    CheckpointProofOutput,
    Option<(CheckpointProofOutput, Proof)>,
) {
    assert_eq!(
        l1_batch.deposits, l2_batch.deposits,
        "Deposits mismatch between L1 and L2"
    );

    // Create BatchInfo based on `l1_batch` and `l2_batch`
    let mut batch_info = BatchInfo::new(
        0,
        (
            l1_batch.initial_snapshot.block_num,
            l1_batch.final_snapshot.block_num,
        ),
        (l2_batch.initial_snapshot.slot, l2_batch.final_snapshot.slot),
        (l1_batch.initial_snapshot.hash, l1_batch.final_snapshot.hash),
        (l2_batch.initial_snapshot.hash, l2_batch.final_snapshot.hash),
        l2_batch.final_snapshot.l2_blockid,
        (
            l1_batch.initial_snapshot.acc_pow,
            l1_batch.final_snapshot.acc_pow,
        ),
    );

    match l1_batch.state_update.as_ref() {
        // If no previous batch info, this means that this is the genesis batch. Set the `curr_idx`
        // as 0 and set the initial information of batch_info as bootstrap.
        None => {
            let bootstrap = batch_info.initial_bootstrap_state();
            (
                CheckpointProofOutput::new(batch_info, bootstrap, vk.to_vec()),
                None,
            )
        }
        Some(prev_checkpoint) => {
            // If some previous state transition, verify that it's sequential
            assert_eq!(
                prev_checkpoint.batch_info().final_bootstrap_state(),
                batch_info.initial_bootstrap_state()
            );

            batch_info.idx = prev_checkpoint.batch_info().idx + 1;

            // Select the bootstrap state for this batch.
            // If there exist proof for the prev_batch, use the prev_batch bootstrap state, else set
            // the current batch initial info as bootstrap
            if prev_checkpoint.proof().is_empty() {
                let output = CheckpointProofOutput::new(
                    batch_info.clone(),
                    batch_info.initial_bootstrap_state(),
                    vk.to_vec(),
                );
                (output, None)
            } else {
                let bootstrap = prev_checkpoint.bootstrap().clone();
                let new_checkpoint_output = CheckpointProofOutput::new(
                    batch_info,
                    prev_checkpoint.bootstrap().clone(),
                    vk.to_vec(),
                );
                let prev_checkpoint_output = CheckpointProofOutput::new(
                    prev_checkpoint.batch_info().clone(),
                    bootstrap.clone(),
                    vk.to_vec(),
                );
                let prev_checkpoint_proof = prev_checkpoint.proof().clone();
                (
                    new_checkpoint_output,
                    Some((prev_checkpoint_output, prev_checkpoint_proof)),
                )
            }
        }
    }
}
