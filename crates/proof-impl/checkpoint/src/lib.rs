//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{params::RollupParams, proof::RollupVerifyingKey};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_proofimpl_l1_batch::L1BatchProofOutput;
use strata_state::batch::{BatchInfo, CheckpointProofOutput};
use strata_zkvm::{Proof, ZkVmEnv};

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

    if let Some(prev_checkpoint) = prev_checkpoint {
        let (checkpoint, proof) = prev_checkpoint;
        zkvm.verify_groth16_proof(&proof, &rollup_vk.0, &borsh::to_vec(&checkpoint).unwrap());
    }

    zkvm.commit_borsh(&output);
}
