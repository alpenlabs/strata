use revm::primitives::SpecId;
use zkvm_primitives::processor::EvmConfig;
use zkvm_primitives::{process_block_transaction, ZKVMInput};

// TODO: Read the evm config from the genesis config. This should be done in compile time.
const EVM_CONFIG: EvmConfig = EvmConfig {
    chain_id: 12345,
    spec_id: SpecId::SHANGHAI,
};

fn main() {
    let input = sp1_zkvm::io::read::<ZKVMInput>();

    // Handle the block validation
    let public_params = process_block_transaction(input, EVM_CONFIG);

    sp1_zkvm::io::commit(&public_params);
}
