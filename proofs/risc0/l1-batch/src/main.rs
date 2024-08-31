use l1_batch::logic::{process_batch_proof, L1BatchProofInput};
use risc0_zkvm::{guest::env, serde};

pub const BTC_BLOCKSPACE_RISC0_ID: [u32; 8] = [
    2306016456, 3255285113, 4013425807, 3434287776, 4100354067, 1869760094, 1016980814, 1706045821,
];

fn main() {
    let input: L1BatchProofInput = env::read();

    for out in &input.batch {
        env::verify(BTC_BLOCKSPACE_RISC0_ID, &serde::to_vec(&out).unwrap()).unwrap();
    }

    let output = process_batch_proof(input);
    env::commit(&output);
}
