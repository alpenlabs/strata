//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{params::RollupParams, proof::RollupVerifyingKey};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_proofimpl_l1_batch::L1BatchProofOutput;
use strata_state::batch::{BatchTransition, CheckpointProofOutput};
use zkaleido::{ProofReceipt, ZkVmEnv};

pub mod prover;

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
) -> (CheckpointProofOutput, Option<ProofReceipt>) {
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
    let batch_transition = BatchTransition::new(
        (
            l1_batch_output.initial_state_hash,
            l1_batch_output.final_state_hash,
        ),
        (
            l2_batch_output.initial_state_hash,
            l2_batch_output.final_state_hash,
        ),
        l1_batch_output.rollup_params_commitment,
    );

    let (checkpoint_base_state, opt_prev_output) = match l1_batch_output.prev_checkpoint.as_ref() {
        // Genesis batch: initialize with initial checkpoint_base_state state
        None => (batch_transition.get_initial_checkpoint_base_state(), None),
        Some(prev_checkpoint) => {
            // Ensure sequential state transition
            assert_eq!(
                prev_checkpoint
                    .batch_transition()
                    .get_final_checkpoint_base_state(),
                batch_transition.get_initial_checkpoint_base_state()
            );

            assert_eq!(
                prev_checkpoint
                    .batch_transition()
                    .rollup_params_commitment(),
                batch_transition.rollup_params_commitment()
            );

            // If there exist proof for the prev_batch, use the prev_batch checkpoint_base_state
            // state, else set the current batch initial info as checkpoint_base_state
            if prev_checkpoint.proof().is_empty() {
                // No proof in previous checkpoint: use initial checkpoint_base_state state
                (batch_transition.get_initial_checkpoint_base_state(), None)
            } else {
                // Use previous checkpoint's checkpoint_base_state state and include previous proof
                let checkpoint_base_state = prev_checkpoint.checkpoint_base_state().clone();
                (
                    checkpoint_base_state,
                    Some(prev_checkpoint.get_proof_receipt()),
                )
            }
        }
    };
    let output = CheckpointProofOutput::new(batch_transition, checkpoint_base_state);
    (output, opt_prev_output)
}

pub fn process_checkpoint_proof_outer(
    zkvm: &impl ZkVmEnv,
    l1_batch_vk: &[u32; 8],
    l2_batch_vk: &[u32; 8],
) {
    let rollup_params: RollupParams = zkvm.read_serde();
    let rollup_vk = match rollup_params.rollup_vk() {
        RollupVerifyingKey::SP1VerifyingKey(sp1_vk) => sp1_vk,
        RollupVerifyingKey::Risc0VerifyingKey(risc0_vk) => risc0_vk,
        RollupVerifyingKey::NativeVerifyingKey(native_vk) => native_vk,
    };

    // verify l1 proof
    let l1_batch_pp = zkvm.read_verified_borsh(l1_batch_vk);
    let l2_batch_pp = zkvm.read_verified_borsh(l2_batch_vk);

    let (output, prev_checkpoint) = process_checkpoint_proof(&l1_batch_pp, &l2_batch_pp);

    if let Some(prev_receipt) = prev_checkpoint {
        zkvm.verify_groth16_receipt(&prev_receipt, &rollup_vk.0);
    }

    zkvm.commit_borsh(&output);
}
