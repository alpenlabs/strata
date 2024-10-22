//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{buf::Buf32, params::RollupParams};
use strata_proofimpl_l1_batch::L1BatchProofOutput;
use strata_state::{
    batch::{BatchInfo, CheckpointProofOutput},
    exec_update::ELDepositData,
    id::L2BlockId,
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
    pub deposits: Vec<ELDepositData>,
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

pub fn process_checkpoint_proof(
    l1_batch_output: &L1BatchProofOutput,
    l2_batch_output: &L2BatchProofOutput,
    rollup_params: &RollupParams,
) -> (
    CheckpointProofOutput,
    Option<(CheckpointProofOutput, Proof)>,
) {
    assert_eq!(
        l1_batch_output.rollup_params_commitment(),
        l2_batch_output.rollup_params_commitment(),
        "Rollup params mismatch between L1 and L2"
    );

    assert_eq!(
        rollup_params.compute_hash(),
        l1_batch_output.rollup_params_commitment(),
        "Rollup params mismatch checkpoint and batches"
    );

    assert_deposits_match(l1_batch_output, l2_batch_output);

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
                );
                let prev_checkpoint_proof = prev_checkpoint.proof().clone();
                (
                    bootstrap,
                    Some((prev_checkpoint_output, prev_checkpoint_proof)),
                )
            }
        }
    };
    let output = CheckpointProofOutput::new(batch_info, bootstrap);
    (output, opt_prev_output)
}

fn assert_deposits_match(
    l1_batch_output: &L1BatchProofOutput,
    l2_batch_output: &L2BatchProofOutput,
) {
    // Assert that the number of deposits is the same in both L1 and L2
    assert_eq!(
        l1_batch_output.deposits.len(),
        l2_batch_output.deposits.len(),
        "Deposits count mismatch between L1 and L2"
    );

    // Iterate over each pair of deposits and assert deposit info
    for (index, (l1_deposit, l2_deposit)) in l1_batch_output
        .deposits
        .iter()
        .zip(l2_batch_output.deposits.iter())
        .enumerate()
    {
        // Assert that the amounts match
        assert_eq!(
            l1_deposit.amt.to_sat(),
            l2_deposit.amt(),
            "Deposit amount mismatch at index {} between L1 and L2",
            index
        );

        // Assert that the addresses match
        assert_eq!(
            l1_deposit.address,
            l2_deposit.dest_addr(),
            "Deposit address mismatch at index {} between L1 and L2",
            index
        );
    }
}
