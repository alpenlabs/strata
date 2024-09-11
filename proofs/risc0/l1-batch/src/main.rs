use bitcoin::params::MAINNET;
use l1_batch::{
    logic::{process_batch_proof, L1BatchProofInput},
    pow_params::PowParams,
};
use risc0_zkvm::{guest::env, serde};

// TODO: read directly from file
// pub const BTC_BLOCKSPACE_RISC0_ID: [u32; 8] = [
//     3924733487, 4261975711, 2287119136, 3197699074, 1661616050, 1659118978, 3476255655,
// 873162380, ];

fn main() {
    let input: L1BatchProofInput = env::read();

    for out in &input.batch {
        let vk: [u32; 8] = env::read();
        env::verify(vk, &serde::to_vec(&out).unwrap()).unwrap();
    }

    let pow_params = PowParams::from(&MAINNET);
    let output = process_batch_proof(input, &pow_params);
    env::commit(&output);
}
