//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::{
    batch::{BootstrapCheckpointInfo, Checkpoint, CheckpointInfo},
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
    pub verifying_key: [u32; 8],
    // TODO: genesis will be hardcoded here
    pub genesis: BootstrapCheckpointInfo,
}

// TODO: genesis needs to be hardcoded
pub fn process_checkpoint_proof(
    l1_batch: &L1BatchProofOutput,
    l2_batch: &L2BatchProofOutput,
    bootstrap: &BootstrapCheckpointInfo,
) -> (Checkpoint, Option<(Checkpoint, Proof)>) {
    let prev_checkpoint = match l1_batch.state_update.as_ref() {
        // If some previous state transition, verify that it's sequential
        Some(prev_checkpoint) => {
            assert_eq!(
                &l1_batch.initial_snapshot.hash,
                prev_checkpoint.checkpoint().final_l1_state_hash(),
                "L1 state mismatch"
            );
            assert_eq!(
                &l2_batch.initial_snapshot.hash,
                prev_checkpoint.checkpoint().final_l2_state_hash(),
                "L2 state mismatch"
            );
            assert_eq!(
                &bootstrap.initial_l1_state,
                prev_checkpoint.checkpoint().initial_l1_state_hash(),
                "L1 state mismatch"
            );
            assert_eq!(
                &bootstrap.initial_l2_state,
                prev_checkpoint.checkpoint().initial_l2_state_hash(),
                "L2 state mismatch"
            );

            Some((
                Checkpoint::new(
                    prev_checkpoint.checkpoint().clone(),
                    prev_checkpoint.bootstrap().clone(),
                ),
                prev_checkpoint.proof().clone(),
            ))
        }
        // If no previous state update, verify against genesis
        None => {
            assert_eq!(
                l1_batch.initial_snapshot.hash, bootstrap.initial_l1_state,
                "L1 genesis mismatch"
            );
            assert_eq!(
                l2_batch.initial_snapshot.hash, bootstrap.initial_l2_state,
                "L2 genesis mismatch"
            );
            None
        }
    };

    assert_eq!(
        l1_batch.deposits, l2_batch.deposits,
        "Deposits mismatch between L1 and L2"
    );

    let checkpoint_idx = prev_checkpoint
        .as_ref()
        .map_or(bootstrap.idx, |(checkpoint, _)| checkpoint.info.idx() + 1);

    let l1_range = (bootstrap.start_l1_height, l1_batch.final_snapshot.block_num);
    let l1_transition = (l1_batch.initial_snapshot.hash, l1_batch.final_snapshot.hash);
    let pow_transition = (
        l1_batch.initial_snapshot.acc_pow,
        l1_batch.final_snapshot.acc_pow,
    );

    let l2_range = (bootstrap.start_l2_height, l2_batch.final_snapshot.slot);
    let l2_transition = (l2_batch.initial_snapshot.hash, l2_batch.final_snapshot.hash);

    let info = CheckpointInfo::new(
        checkpoint_idx,
        l1_range,
        l2_range,
        l1_transition,
        l2_transition,
        l2_batch.final_snapshot.l2_blockid,
        pow_transition,
    );

    let output = Checkpoint::new(info, bootstrap.clone());

    (output, prev_checkpoint)
}
