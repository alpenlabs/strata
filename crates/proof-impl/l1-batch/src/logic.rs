use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::{
    batch::Checkpoint,
    buf::Buf32,
    l1::{DepositInfo, HeaderVerificationState},
    params::RollupParams,
};
use zkaleido::ZkVmEnv;

/// Represents the public parameters of the L1BlockScan batch proof.
#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct L1BatchProofOutput {
    pub deposits: Vec<DepositInfo>,
    pub prev_checkpoint: Option<Checkpoint>,
    pub initial_state_hash: Buf32,
    pub final_state_hash: Buf32,
    pub rollup_params_commitment: Buf32,
}

impl L1BatchProofOutput {
    pub fn rollup_params_commitment(&self) -> Buf32 {
        self.rollup_params_commitment
    }
}

pub fn process_l1_batch_proof(zkvm: &impl ZkVmEnv) {
    let state: HeaderVerificationState = zkvm.read_borsh();

    let rollup_params: RollupParams = zkvm.read_serde();

    let num_inputs: u32 = zkvm.read_serde();
    assert!(num_inputs > 0);

    let initial_state_hash = state.compute_hash().expect("failed to compute state hash");
    let deposits = Vec::new();
    let prev_checkpoint = None;

    // FIXME: remove this crate once other things are cleaned up

    let final_state_hash = state.compute_hash().expect("failed to compute state hash");

    let output = L1BatchProofOutput {
        deposits,
        prev_checkpoint,
        initial_state_hash,
        final_state_hash,
        rollup_params_commitment: rollup_params.compute_hash(),
    };

    zkvm.commit_borsh(&output);
}
