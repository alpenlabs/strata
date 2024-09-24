use bitcoin::params::MAINNET;
use express_proofimpl_l1_batch::{
    logic::{process_batch_proof, L1BatchProofInput},
    pow_params::PowParams,
};
use sha2::{Digest, Sha256};

fn main() {
    let input_raw = sp1_zkvm::io::read_vec();
    let input: L1BatchProofInput = borsh::from_slice(&input_raw).unwrap();

    for out in &input.batch {
        let vk: [u32; 8] = sp1_zkvm::io::read();
        let out_raw = bincode::serialize(&borsh::to_vec(out).unwrap()).unwrap();
        let public_values_digest = Sha256::digest(&out_raw);
        sp1_zkvm::lib::verify::verify_sp1_proof(&vk, &public_values_digest.into());
    }

    let pow_params = PowParams::from(&MAINNET);
    let output = process_batch_proof(input, &pow_params);

    sp1_zkvm::io::commit(&borsh::to_vec(&output).unwrap());
}
