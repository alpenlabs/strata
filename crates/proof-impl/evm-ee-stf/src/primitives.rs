use std::collections::HashMap;

use alloy_consensus::{serde_bincode_compat, Header};
use reth_primitives::{
    revm_primitives::alloy_primitives::{Address, Bytes, FixedBytes, B256},
    TransactionSignedNoHash, Withdrawal,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use strata_reth_primitives::WithdrawalIntent;
use strata_state::block::ExecSegment;

use crate::mpt::{MptNode, StorageEntry};

/// Public Parameters that proof asserts
pub type ElProofOutput = Vec<ExecSegment>;

/// Public Parameters that proof asserts
pub type ElProofInput = Vec<ElBlockStfInput>;

/// Result of the block execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElBlockStfOutput {
    pub block_idx: u64,
    pub prev_blockhash: FixedBytes<32>,
    pub new_blockhash: FixedBytes<32>,
    pub new_state_root: FixedBytes<32>,
    pub txn_root: FixedBytes<32>,
    pub withdrawal_intents: Vec<WithdrawalIntent>,
    pub deposits_txns_root: FixedBytes<32>,
}

#[serde_as]
/// Necessary information to prove the execution of a EL block.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElBlockStfInput {
    /// The Keccak 256-bit hash of the parent block's header, in its entirety.
    /// N.B. The reason serde_bincode_compat is necessary:
    /// `[serde_bincode_compat]`(alloy_consensus::serde_bincode_compat)
    #[serde_as(as = "serde_bincode_compat::Header")]
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
