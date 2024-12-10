// This code is modified from the original implementation of Zeth.
//
// Reference: https://github.com/risc0/zeth
//
// Copyright 2023 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either strata or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
pub mod db;
pub mod mpt;
pub mod primitives;
pub mod processor;
pub mod prover;
use db::InMemoryDBHelper;
use mpt::keccak;
pub use primitives::{ELProofInput, ELProofPublicParams};
use processor::{EvmConfig, EvmProcessor};
use reth_primitives::revm_primitives::alloy_primitives::B256;
use revm::{primitives::SpecId, InMemoryDB};
use strata_primitives::{buf::Buf32, evm_exec::create_evm_extra_payload};
use strata_reth_evm::collect_withdrawal_intents;
use strata_state::{
    block::ExecSegment,
    exec_update::{ExecUpdate, UpdateInput, UpdateOutput},
};
use strata_zkvm::ZkVmEnv;

// TODO: Read the evm config from the genesis config. This should be done in compile time.
const EVM_CONFIG: EvmConfig = EvmConfig {
    chain_id: 12345,
    spec_id: SpecId::SHANGHAI,
};
/// Executes the block with the given input and EVM configuration, returning public parameters.
pub fn process_block_transaction(
    mut input: ELProofInput,
    evm_config: EvmConfig,
) -> ELProofPublicParams {
    // Calculate the previous block hash
    let previous_block_hash = B256::from(keccak(alloy_rlp::encode(input.parent_header.clone())));

    // Initialize the in-memory database
    let db = match InMemoryDB::initialize(&mut input) {
        Ok(database) => database,
        Err(e) => panic!("Failed to initialize database: {:?}", e),
    };

    // Create an EVM processor and execute the block
    let mut evm_processor = EvmProcessor::<InMemoryDB> {
        input,
        db: Some(db),
        header: None,
        evm_config,
    };

    evm_processor.initialize();
    let receipts = evm_processor.execute();
    evm_processor.finalize();

    // Extract the header and compute the new block hash
    let block_header = evm_processor.header.unwrap(); // Ensure header exists before unwrap
    let new_block_hash = B256::from(keccak(alloy_rlp::encode(block_header.clone())));

    // TODO: Optimize receipt iteration by implementing bloom filters or adding hints to
    // `ELProofInput`. This will allow for efficient filtering of`WithdrawalIntentEvents`.
    let withdrawal_intents =
        collect_withdrawal_intents(receipts.into_iter().map(|el| Some(el.receipt)))
            .collect::<Vec<_>>();

    // Construct the public parameters for the proof
    ELProofPublicParams {
        block_idx: block_header.number,
        new_blockhash: new_block_hash,
        new_state_root: block_header.state_root,
        prev_blockhash: previous_block_hash,
        txn_root: block_header.transactions_root,
        deposits_txns_root: block_header.withdrawals_root.unwrap_or_default(),
        withdrawal_intents,
    }
}

pub fn process_block_transaction_outer(zkvm: &impl ZkVmEnv) {
    let total_blocks: u32 = zkvm.read_serde();
    assert!(total_blocks > 0, "At least one block is required.");

    let mut exec_updates = Vec::with_capacity((total_blocks - 1) as usize);

    // Process the first block
    let first_input: ELProofInput = zkvm.read_serde();
    let first_output = process_block_transaction(first_input, EVM_CONFIG);
    let mut current_blockhash = first_output.new_blockhash;

    // Process remaining blocks
    for _ in 1..total_blocks {
        let next_block_input: ELProofInput = zkvm.read_serde();
        let next_block_output = process_block_transaction(next_block_input, EVM_CONFIG);
        assert_eq!(next_block_output.prev_blockhash, current_blockhash);

        let exec_segment = generate_exec_update(&next_block_output);
        exec_updates.push(exec_segment);

        current_blockhash = next_block_output.new_blockhash;
    }

    zkvm.commit_borsh(&exec_updates);
}

/// Generates an execution segment from the given ELProof public parameters.
pub fn generate_exec_update(el_proof_pp: &ELProofPublicParams) -> ExecSegment {
    // create_evm_extra_payload
    let update_input = UpdateInput::new(
        el_proof_pp.block_idx,
        Vec::new(),
        Buf32(*el_proof_pp.txn_root),
        create_evm_extra_payload(Buf32(*el_proof_pp.new_blockhash)),
    );

    let update_output = UpdateOutput::new_from_state(Buf32(*el_proof_pp.new_state_root));
    let exec_update = ExecUpdate::new(update_input, update_output);

    ExecSegment::new(exec_update)
}

#[cfg(test)]
mod tests {
    use revm::primitives::SpecId;
    use serde::{Deserialize, Serialize};

    use super::*;
    const EVM_CONFIG: EvmConfig = EvmConfig {
        chain_id: 12345,
        spec_id: SpecId::SHANGHAI,
    };

    #[derive(Serialize, Deserialize)]
    struct TestData {
        witness: ELProofInput,
        params: ELProofPublicParams,
    }

    fn get_mock_data() -> TestData {
        let json_content = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("test_data/witness_params.json"),
        )
        .expect("Failed to read the blob data file");

        serde_json::from_str(&json_content).expect("Valid json")
    }

    #[test]
    fn basic_serde() {
        // Checks that serialization and deserialization actually works.
        let test_data = get_mock_data();

        let s = bincode::serialize(&test_data.witness).unwrap();
        let d: ELProofInput = bincode::deserialize(&s[..]).unwrap();
        assert_eq!(d, test_data.witness);
    }

    #[test]
    fn block_stf_test() {
        let test_data = get_mock_data();

        let input = test_data.witness;
        let op = process_block_transaction(input, EVM_CONFIG);
        assert_eq!(op, test_data.params);
    }
}
