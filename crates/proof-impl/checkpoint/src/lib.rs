//! This crate implements the final batch proof that aggregates both L1 Batch Proof and L2 Batch
//! Proof. It ensures that the previous batch proof was correctly settled on the L1
//! chain and that all L1-L2 transactions were processed.

use borsh::{BorshDeserialize, BorshSerialize};
use strata_proofimpl_cl_stf::L2BatchProofOutput;
use strata_state::batch::CheckpointProofOutput;
use zkaleido::ZkVmEnv;

pub mod prover;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CheckpointProofInput {
    pub l2_state: L2BatchProofOutput,
    /// The verifying key of this checkpoint program.
    /// Required for verifying the Groth16 proof of this program.
    /// Cannot be hardcoded as any change to the program or proof implementation
    /// will change verifying_key.
    pub vk: Vec<u8>,
}

pub fn process_checkpoint_proof_outer(zkvm: &impl ZkVmEnv, l2_batch_vk: &[u32; 8]) {
    // verify l1 proof
    let l2_batch_pp: L2BatchProofOutput = zkvm.read_verified_borsh(l2_batch_vk);

    zkvm.commit_borsh(&l2_batch_pp);
}
