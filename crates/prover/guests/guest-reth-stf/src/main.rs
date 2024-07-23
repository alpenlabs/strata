use risc0_zkvm::guest::env;
use zkvm_primitives::SP1RethInput;

const ENCODED_IP: &[u8] = include_bytes!("../1.bin");

fn main() {
    // TODO: Implement your guest code here
    let witness: SP1RethInput = bincode::deserialize(ENCODED_IP).unwrap();

    // read the input
    let input: u32 = env::read();

    // TODO: do something with the input

    // write public output to the journal
    env::commit(&input);
}
