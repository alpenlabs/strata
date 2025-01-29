use strata_proofimpl_cl_stf::L2BatchProofOutput;
use zkaleido::ZkVmEnv;

mod prover;
pub use prover::*;

pub fn process_cl_agg(zkvm: &impl ZkVmEnv, cl_stf_vk: &[u32; 8]) {
    let num_agg_inputs: u32 = zkvm.read_serde();
    assert!(
        num_agg_inputs >= 1,
        "At least one CL proof is required for aggregation"
    );

    let cl_proof_pp_start: L2BatchProofOutput = zkvm.read_verified_borsh(cl_stf_vk);
    let mut cl_proof_pp_prev = cl_proof_pp_start.clone();
    let mut acc_deposits = cl_proof_pp_start.deposits.clone();

    let rollup_params_commitment = cl_proof_pp_start.rollup_params_commitment();

    for _ in 0..(num_agg_inputs - 1) {
        let next_proof_pp = zkvm.read_verified_borsh(cl_stf_vk);
        validate_proof_consistency(&cl_proof_pp_prev, &next_proof_pp);
        assert_eq!(
            rollup_params_commitment,
            next_proof_pp.rollup_params_commitment()
        );
        acc_deposits.extend(next_proof_pp.deposits.clone());
        cl_proof_pp_prev = next_proof_pp;
    }

    // Combine the initial state root from the first proof and the post-state root from the last
    // proof of the batch
    let public_params = L2BatchProofOutput {
        deposits: acc_deposits,
        initial_state_hash: cl_proof_pp_start.initial_state_hash,
        final_state_hash: cl_proof_pp_prev.final_state_hash,
        rollup_params_commitment,
    };

    zkvm.commit_borsh(&public_params);
}

#[inline]
fn validate_proof_consistency(
    current_proof_cs_snap: &L2BatchProofOutput,
    next_proof_cs_snap: &L2BatchProofOutput,
) {
    assert_eq!(
        current_proof_cs_snap.final_state_hash, // post-state root of the current proof
        next_proof_cs_snap.initial_state_hash,  // initial state root of the next proof
        "State root mismatch between proofs"
    );
}
