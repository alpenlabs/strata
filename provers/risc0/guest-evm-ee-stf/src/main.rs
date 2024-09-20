use strata_proofimpl_evm_ee_stf::{process_block_transaction, processor::EvmConfig, ELProofInput};
use revm::primitives::SpecId;
use risc0_zkvm::guest::env;

// TODO: Read the evm config from the genesis config. This should be done in compile time.
const EVM_CONFIG: EvmConfig = EvmConfig {
    chain_id: 12345,
    spec_id: SpecId::SHANGHAI,
};

fn main() {
    let input: ELProofInput = env::read();

    // Handle the block validation
    let public_params = process_block_transaction(input, EVM_CONFIG);

    env::commit(&public_params);
}
