use strata_proofimpl_l1_batch::{
    logic::{process_batch_proof, L1BatchProofInput},
    params::get_btc_params,
};
use risc0_zkvm::guest::env;

// TODO: read vk for BTC_BLOCKSPACE from a file as this changes
// Ref: https://github.com/risc0/risc0/blob/main/examples/composition/src/main.rs#L15

fn main() {
    let len: u32 = env::read();
    let mut slice = vec![0u8; len as usize];
    env::read_slice(&mut slice);
    let input: L1BatchProofInput = borsh::from_slice(&slice).unwrap();

    for out in &input.batch {
        // TODO: hardcode vk for BTC_BLOCKSPACE
        let vk: [u32; 8] = env::read();
        let out_raw = borsh::to_vec(out).unwrap();
        env::verify(vk, &out_raw).unwrap();
    }

    let output = process_batch_proof(input, &get_btc_params());
    env::commit(&borsh::to_vec(&output).unwrap());
}
