use bitcoin::params::MAINNET;
use l1_batch::{
    logic::{process_batch_proof, L1BatchProofInput},
    pow_params::PowParams,
};
use risc0_zkvm::{guest::env, serde};

// TODO: read vk for BTC_BLOCKSPACE from a file as this changes
// Ref: https://github.com/risc0/risc0/blob/main/examples/composition/src/main.rs#L15

fn main() {
    let input: L1BatchProofInput = env::read();

    for out in &input.batch {
        // TODO: hardcode vk for BTC_BLOCKSPACE
        let vk: [u32; 8] = env::read();
        env::verify(vk, &serde::to_vec(&out).unwrap()).unwrap();
    }

    let pow_params = PowParams::from(&MAINNET);
    let output = process_batch_proof(input, &pow_params);
    env::commit(&output);
}
