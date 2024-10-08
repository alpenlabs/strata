//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::buf::Buf32;
use strata_proofimpl_l1_batch::L1BatchProofOutput;
use strata_state::{
    batch::{BatchInfo, BootstrapState},
    id::L2BlockId,
    tx::DepositInfo,
};
use strata_zkvm::Proof;

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
    pub rollup_params_commitment: Buf32,
}

impl L2BatchProofOutput {
    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
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
    pub bootstrap_state: BootstrapState,
    pub rollup_params_commitment: Buf32,
}

impl CheckpointProofOutput {
    pub fn new(
        info: BatchInfo,
        bootstrap: BootstrapState,
        rollup_params_commitment: Buf32,
    ) -> CheckpointProofOutput {
        Self {
            info,
            bootstrap_state: bootstrap,
            rollup_params_commitment,
        }
    }
}

pub fn process_checkpoint_proof(
    l1_batch_output: &L1BatchProofOutput,
    l2_batch_output: &L2BatchProofOutput,
) -> (
    CheckpointProofOutput,
    Option<(CheckpointProofOutput, Proof)>,
) {
    assert_eq!(
        l1_batch_output.deposits, l2_batch_output.deposits,
        "Deposits mismatch between L1 and L2"
    );

    assert_eq!(
        l1_batch_output.rollup_params_commitment(),
        l2_batch_output.rollup_params_commitment(),
        "Rollup params mismatch between L1 and L2"
    );

    // Create BatchInfo based on `l1_batch` and `l2_batch`
    let mut batch_info = BatchInfo::new(
        0,
        (
            l1_batch_output.initial_snapshot.block_num,
            l1_batch_output.final_snapshot.block_num,
        ),
        (
            l2_batch_output.initial_snapshot.slot,
            l2_batch_output.final_snapshot.slot,
        ),
        (
            l1_batch_output.initial_snapshot.hash,
            l1_batch_output.final_snapshot.hash,
        ),
        (
            l2_batch_output.initial_snapshot.hash,
            l2_batch_output.final_snapshot.hash,
        ),
        l2_batch_output.final_snapshot.l2_blockid,
        (
            l1_batch_output.initial_snapshot.acc_pow,
            l1_batch_output.final_snapshot.acc_pow,
        ),
        l1_batch_output.rollup_params_commitment,
    );

    let (bootstrap, opt_prev_output) = match l1_batch_output.prev_checkpoint.as_ref() {
        // Genesis batch: initialize with initial bootstrap state
        None => (batch_info.get_initial_bootstrap_state(), None),
        Some(prev_checkpoint) => {
            // Ensure sequential state transition
            assert_eq!(
                prev_checkpoint.batch_info().get_final_bootstrap_state(),
                batch_info.get_initial_bootstrap_state()
            );

            assert_eq!(
                prev_checkpoint.batch_info().rollup_params_commitment(),
                batch_info.rollup_params_commitment()
            );

            batch_info.idx = prev_checkpoint.batch_info().idx + 1;

            // If there exist proof for the prev_batch, use the prev_batch bootstrap state, else set
            // the current batch initial info as bootstrap
            if prev_checkpoint.proof().is_empty() {
                // No proof in previous checkpoint: use initial bootstrap state
                (batch_info.get_initial_bootstrap_state(), None)
            } else {
                // Use previous checkpoint's bootstrap state and include previous proof
                let bootstrap = prev_checkpoint.bootstrap_state().clone();
                let prev_checkpoint_output = CheckpointProofOutput::new(
                    prev_checkpoint.batch_info().clone(),
                    bootstrap.clone(),
                    l1_batch_output.rollup_params_commitment,
                );
                let prev_checkpoint_proof = prev_checkpoint.proof().clone();
                (
                    bootstrap,
                    Some((prev_checkpoint_output, prev_checkpoint_proof)),
                )
            }
        }
    };
    let output = CheckpointProofOutput::new(
        batch_info,
        bootstrap,
        l1_batch_output.rollup_params_commitment,
    );
    (output, opt_prev_output)
}
