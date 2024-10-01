use express_proofimpl_cl_stf::CLProofPublicParams;
use sha2::{Digest, Sha256};

mod vks;

fn main() {
    let num_agg_inputs: u32 = sp1_zkvm::io::read();
    assert!(
        num_agg_inputs >= 1,
        "At least one CL proof is required for aggregation"
    );

    let cl_proof_pp_start = read_and_validate_next_proof();
    let mut cl_proof_pp_prev = CLProofPublicParams {
        prev_state_root: cl_proof_pp_start.prev_state_root,
        new_state_root: cl_proof_pp_start.new_state_root,
    };

    for _ in 0..num_agg_inputs - 1 {
        let next_proof_pp = read_and_validate_next_proof();
        validate_proof_consistency(&cl_proof_pp_prev, &next_proof_pp);
        cl_proof_pp_prev = next_proof_pp;
    }

    // Combine the initial state root from the first proof and the post-state root from the last
    // proof of the batch
    let public_params = CLProofPublicParams {
        prev_state_root: cl_proof_pp_start.prev_state_root,
        new_state_root: cl_proof_pp_prev.new_state_root,
    };
    sp1_zkvm::io::commit(&public_params);
}

fn read_and_validate_next_proof() -> CLProofPublicParams {
    // TODO: AggProofInput avoid wiriting vkey to guest.
    // vkey is already embedded to the guest
    let _ = sp1_zkvm::io::read::<[u32; 8]>();
    let cl_block_vkey = vks::GUEST_CL_STF_ELF_ID;
    let cl_proof_pp: Vec<u8> = sp1_zkvm::io::read();

    // Verify the CL block proof
    let public_values_digest = Sha256::digest(&cl_proof_pp);
    sp1_zkvm::lib::verify::verify_sp1_proof(cl_block_vkey, &public_values_digest.into());

    let cl_proof_pp_deserialized: CLProofPublicParams = bincode::deserialize(&cl_proof_pp).unwrap();
    cl_proof_pp_deserialized
}

fn validate_proof_consistency(
    current_proof_pp: &CLProofPublicParams,
    next_proof_pp: &CLProofPublicParams,
) {
    assert_eq!(
        current_proof_pp.new_state_root, // post-state root of the current proof
        next_proof_pp.prev_state_root,   // initial state root of the next proof
        "State root mismatch between proofs"
    );
}
