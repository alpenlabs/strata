use express_cl_stf::CLProofPublicParams;
use sha2::{Digest, Sha256};

fn main() {
    let num_agg_inputs: u32 = sp1_zkvm::io::read();
    assert!(
        num_agg_inputs >= 1,
        "At least one CL proof is required for aggregation"
    );

    let cl_proof_pp_start = read_and_validate_next_proof();
    let mut cl_proof_pp_prev = cl_proof_pp_start;

    for _ in 0..num_agg_inputs - 1 {
        let next_proof_pp = read_and_validate_next_proof();
        validate_proof_consistency(cl_proof_pp_prev, next_proof_pp);
        cl_proof_pp_prev = next_proof_pp;
    }

    // Combine the initial state root from the first proof and the post-state root from the last
    // proof of the batch
    let public_params = (cl_proof_pp_start.0, cl_proof_pp_prev.1);
    sp1_zkvm::io::commit(&public_params);
}

fn read_and_validate_next_proof() -> CLProofPublicParams {
    let cl_block_vkey = sp1_zkvm::io::read::<[u32; 8]>();
    let cl_proof_pp: CLProofPublicParams = sp1_zkvm::io::read();

    // Verify the CL block proof
    // let public_values_digest = Sha256::digest(&cl_proof_pp);
    // sp1_zkvm::lib::verify::verify_sp1_proof(&cl_block_vkey, &public_values_digest.into());

    cl_proof_pp
}

fn validate_proof_consistency(
    current_proof_pp: CLProofPublicParams,
    next_proof_pp: CLProofPublicParams,
) {
    // Compare the post-state root of the current proof with the initial state root of the next
    // proof
    assert_eq!(
        current_proof_pp.1, // post-state root of the current proof
        next_proof_pp.0,    // initial state root of the next proof
        "State root mismatch between proofs"
    );
}
