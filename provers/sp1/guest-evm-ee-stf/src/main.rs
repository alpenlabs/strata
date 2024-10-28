use revm::primitives::SpecId;
use strata_proofimpl_evm_ee_stf::{process_block_transaction, processor::EvmConfig, ELProofInput};

// TODO: Read the evm config from the genesis config. This should be done in compile time.
const EVM_CONFIG: EvmConfig = EvmConfig {
    chain_id: 8091,
    spec_id: SpecId::SHANGHAI,
};

fn main() {
    let input = sp1_zkvm::io::read::<ELProofInput>();

    // Handle the block validation
    let public_params = process_block_transaction(input, EVM_CONFIG);

    sp1_zkvm::io::commit(&public_params);
}
