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
pub mod processor;

use std::collections::HashMap;

use db::InMemoryDBHelper;
use mpt::keccak;
use processor::{EvmConfig, EvmProcessor};
use reth_primitives::{
    alloy_primitives::FixedBytes, Address, Bytes, Header, TransactionSignedNoHash, Withdrawal, B256,
};
use revm::InMemoryDB;
use serde::{Deserialize, Serialize};

use crate::mpt::{MptNode, StorageEntry};

/// Public Parameters that proof asserts
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ELProofPublicParams {
    pub block_idx: u64,
    pub prev_blockhash: FixedBytes<32>,
    pub new_blockhash: FixedBytes<32>,
    pub new_state_root: FixedBytes<32>,
    pub txn_root: FixedBytes<32>,
    pub withdrawals: Vec<FixedBytes<32>>,
}

/// Necessary information to prove the execution of the RETH block.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ELProofInput {
    /// The Keccak 256-bit hash of the parent block's header, in its entirety.
    pub parent_header: Header,

    /// The 160-bit address to which all fees collected from the successful mining of this block
    /// be transferred.
    pub beneficiary: Address,

    /// A scalar value equal to the current limit of gas expenditure per block.
    pub gas_limit: u64,

    /// A scalar value equal to the reasonable output of Unix's time() at this block's inception.
    pub timestamp: u64,

    /// An arbitrary byte array containing data relevant to this block. This must be 32 bytes or
    /// fewer.
    pub extra_data: Bytes,

    /// A 256-bit hash which, combined with the nonce, proves that a sufficient amount of
    /// computation has been carried out on this block.
    pub mix_hash: B256,

    /// The state trie of the parent block.
    pub parent_state_trie: MptNode,

    /// The storage of the parent block.
    pub parent_storage: HashMap<Address, StorageEntry>,

    /// The relevant contracts for the block.
    pub contracts: Vec<Bytes>,

    /// The ancestor headers of the parent block.
    pub ancestor_headers: Vec<Header>,

    /// A list of transactions to process.
    pub transactions: Vec<TransactionSignedNoHash>,

    /// A list of withdrawals to process.
    pub withdrawals: Vec<Withdrawal>,
}

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
    evm_processor.execute();
    evm_processor.finalize();

    // Extract the header and compute the new block hash
    let block_header = evm_processor.header.unwrap(); // Ensure header exists before unwrap
    let new_block_hash = B256::from(keccak(alloy_rlp::encode(block_header.clone())));

    // Construct the public parameters for the proof
    ELProofPublicParams {
        block_idx: block_header.number,
        new_blockhash: new_block_hash,
        new_state_root: block_header.state_root,
        prev_blockhash: previous_block_hash,
        txn_root: block_header.transactions_root,
        withdrawals: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use revm::primitives::SpecId;

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

    #[test]
    fn block_stf_test() {
        let json_content = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_data/witness_params.json"),
        )
        .expect("Failed to read the blob data file");

        let test_data: TestData = serde_json::from_str(&json_content).expect("failed");
        let input = test_data.witness;
        let op = process_block_transaction(input, EVM_CONFIG);

        assert_eq!(op, test_data.params);
    }
}
