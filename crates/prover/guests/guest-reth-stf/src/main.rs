use risc0_zkvm::guest::env;

use revm::primitives::SpecId;
use zkvm_primitives::processor::EvmConfig;
use zkvm_primitives::{process_block_transaction, ZKVMInput};

const ENCODED_INPUT: &[u8] = include_bytes!("../1.bin");

// TODO: Read the evm config from the genesis config. This should be done in compile time.
const EVM_CONFIG: EvmConfig = EvmConfig {
    chain_id: 12345,
    spec_id: SpecId::SHANGHAI,
};

fn main() {
    // TODO: Read the input from the host
    let input: ZKVMInput = bincode::deserialize(ENCODED_INPUT).unwrap();

    // Handle the block validation
    let public_params = process_block_transaction(input, EVM_CONFIG);

    env::commit(&public_params);
}
