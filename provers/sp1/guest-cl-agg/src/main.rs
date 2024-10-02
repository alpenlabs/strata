use express_proofimpl_checkpoint::L2BatchProofOutput;
use sha2::{Digest, Sha256};

mod vks;

fn main() {
    let num_agg_inputs: u32 = sp1_zkvm::io::read();
    assert!(
        num_agg_inputs >= 1,
        "At least one CL proof is required for aggregation"
    );

    let cl_proof_pp_start = read_and_validate_next_proof();
    let mut cl_proof_pp_prev = cl_proof_pp_start.clone();
    let mut acc_deposits = cl_proof_pp_start.deposits.clone();

    for _ in 0..num_agg_inputs - 1 {
        let next_proof_pp = read_and_validate_next_proof();
        validate_proof_consistency(&cl_proof_pp_prev, &next_proof_pp);
        acc_deposits.extend(next_proof_pp.deposits.clone());
        cl_proof_pp_prev = next_proof_pp;
    }

    // Combine the initial state root from the first proof and the post-state root from the last
    // proof of the batch
    let public_params = L2BatchProofOutput {
        deposits: acc_deposits,
        initial_snapshot: cl_proof_pp_start.initial_snapshot,
        final_snapshot: cl_proof_pp_prev.final_snapshot,
    };

    sp1_zkvm::io::commit(&borsh::to_vec(&public_params).unwrap());
}

fn read_and_validate_next_proof() -> L2BatchProofOutput {
    // TODO: AggProofInput avoid wiriting vkey to guest.
    // vkey is already embedded to the guest
    let _ = sp1_zkvm::io::read::<[u32; 8]>();
    let cl_block_vkey = vks::GUEST_CL_STF_ELF_ID;
    let cl_proof_pp: Vec<u8> = sp1_zkvm::io::read();

    // Verify the CL block proof
    let public_values_digest = Sha256::digest(&cl_proof_pp);
    sp1_zkvm::lib::verify::verify_sp1_proof(cl_block_vkey, &public_values_digest.into());

    let cl_proof_pp_serialized: Vec<u8> = bincode::deserialize(&cl_proof_pp).unwrap();
    borsh::from_slice(&cl_proof_pp_serialized).unwrap()
}

fn validate_proof_consistency(
    current_proof_cs_snap: &L2BatchProofOutput,
    next_proof_cs_snap: &L2BatchProofOutput,
) {
    assert_eq!(
        current_proof_cs_snap.final_snapshot.hash, // post-state root of the current proof
        next_proof_cs_snap.initial_snapshot.hash,  // initial state root of the next proof
        "State root mismatch between proofs"
    );
}
