//! Core logic of the Bitcoin Blockspace proof that will be proven

use std::str::FromStr;

use bitcoin::{block::Header, Address, Block};
use serde::{Deserialize, Serialize};

use crate::{
    block::{check_merkle_root, check_witness_commitment},
    filter::{extract_relevant_transactions, Deposit, ForcedInclusion, StateUpdate},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockspaceProofInput {
    pub block: Block,
    // TODO: figure out why serialize `Address`
    pub bridge_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockspaceProofOutput {
    pub header: Header,
    pub deposits: Vec<Deposit>,
    pub forced_inclusions: Vec<ForcedInclusion>,
    pub state_updates: Vec<StateUpdate>,
}

pub fn process_blockspace_proof(input: &BlockspaceProofInput) -> BlockspaceProofOutput {
    let BlockspaceProofInput {
        block,
        bridge_address,
    } = input;
    assert!(check_merkle_root(block));
    assert!(check_witness_commitment(block));

    // TODO: understand the implication
    let bridge_address = Address::from_str(bridge_address).unwrap().assume_checked();

    let (deposits, forced_inclusions, state_updates) =
        extract_relevant_transactions(block, &bridge_address);

    BlockspaceProofOutput {
        header: block.header,
        deposits,
        forced_inclusions,
        state_updates,
    }
}
