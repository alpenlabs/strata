//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use alpen_express_primitives::buf::Buf32;
use alpen_express_state::{
    batch::{
        BootstrapCheckpoint, Checkpoint, CheckpointInfo, CheckpointTransition, StateTransition,
    },
    id::L2BlockId,
    tx::DepositInfo,
};
use borsh::{BorshDeserialize, BorshSerialize};
use express_proofimpl_l1_batch::logic::L1BatchProofOutput;
use express_zkvm::Proof;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct ChainStateSnapshot {
    pub hash: Buf32,
    pub slot: u64,
    pub l2_blockid: L2BlockId,
}

#[derive(Debug, BorshSerialize, BorshDeserialize)]
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
    pub genesis: BootstrapCheckpoint,
}

// TODO: genesis needs to be hardcoded
pub fn process_checkpoint_proof(
    l1_batch: &L1BatchProofOutput,
    l2_batch: &L2BatchProofOutput,
    bootstrap: &BootstrapCheckpoint,
) -> (Checkpoint, Option<(Checkpoint, Proof)>) {
    let prev_checkpoint = match l1_batch.state_update.as_ref() {
        // If no some previous state transition, verify that it's sequential
        Some(prev_checkpoint) => {
            let prev_transition = prev_checkpoint.checkpoint().transition();

            assert_eq!(
                &l1_batch.initial_snapshot.hash,
                prev_transition.final_l1_state_hash(),
                "L1 state mismatch"
            );
            assert_eq!(
                &l2_batch.initial_snapshot.hash,
                prev_transition.final_l2_state_hash(),
                "L2 state mismatch"
            );

            Some((
                prev_checkpoint.checkpoint().clone(),
                prev_checkpoint.proof().clone(),
            ))
        }
        // If no previous state update, verify against genesis
        None => {
            assert_eq!(
                l1_batch.initial_snapshot.hash, bootstrap.state.l1_state_hash,
                "L1 genesis mismatch"
            );
            assert_eq!(
                l2_batch.initial_snapshot.hash, bootstrap.state.l2_state_hash,
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
        .map_or(bootstrap.info.idx, |(checkpoint, _)| checkpoint.idx() + 1);

    let l1_range = (
        bootstrap.info.start_l1_height,
        l1_batch.final_snapshot.block_num,
    );
    let l2_range = (bootstrap.info.start_l2_height, l2_batch.final_snapshot.slot);

    let info = CheckpointInfo::new(
        checkpoint_idx,
        l1_range,
        l2_range,
        l2_batch.final_snapshot.l2_blockid,
    );

    let state = CheckpointTransition::new(
        StateTransition::new(l1_batch.initial_snapshot.hash, l1_batch.final_snapshot.hash),
        StateTransition::new(l2_batch.initial_snapshot.hash, l2_batch.final_snapshot.hash),
        l1_batch.final_snapshot.acc_pow,
    );

    let output = Checkpoint::new(info, state);

    (output, prev_checkpoint)
}
