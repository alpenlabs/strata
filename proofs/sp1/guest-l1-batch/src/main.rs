use bitcoin::params::MAINNET;
use l1_batch::{
    logic::{process_batch_proof, L1BatchProofInput},
    pow_params::PowParams,
};
use sha2::{Digest, Sha256};

fn main() {
    let input = sp1_zkvm::io::read::<L1BatchProofInput>();

    for out in &input.batch {
        let vk: [u32; 8] = sp1_zkvm::io::read();
        let public_values_digest = Sha256::digest(bincode::serialize(out).unwrap());
        sp1_zkvm::lib::verify::verify_sp1_proof(&vk, &public_values_digest.into());
    }

    let pow_params = PowParams::from(&MAINNET);
    let output = process_batch_proof(input, &pow_params);

    sp1_zkvm::io::commit(&output);
}
