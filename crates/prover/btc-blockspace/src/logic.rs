//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{block::Header, Block};
use serde::{Deserialize, Serialize};

use crate::{
    block::check_merkle_root,
    filter::{extract_relevant_transactions, Deposit, ForcedInclusion, StateUpdate},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockspaceProofInput {
    pub block: Block,
    pub scan_params: ScanParams,
    // TODO: add hintings and other necessary params
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanParams {
    // TODO: figure out why serialize `Address`
    pub bridge_address: String,
    pub sequencer_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockspaceProofOutput {
    pub header: Header,
    pub deposits: Vec<Deposit>,
    pub forced_inclusions: Vec<ForcedInclusion>,
    pub state_updates: Vec<StateUpdate>,
}

pub fn process_blockspace_proof(input: &BlockspaceProofInput) -> BlockspaceProofOutput {
    let BlockspaceProofInput { block, scan_params } = input;
    assert!(check_merkle_root(block));
    // assert!(check_witness_commitment(block));

    let (deposits, forced_inclusions, state_updates) =
        extract_relevant_transactions(block, scan_params);

    BlockspaceProofOutput {
        header: block.header,
        deposits,
        forced_inclusions,
        state_updates,
    }
}
