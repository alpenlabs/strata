use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::l1::{BitcoinAmount, OutputRef};

use crate::batch::SignedBatchCheckpoint;

/// Information related to relevant transactions to be stored in L1Tx
#[derive(
    Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary, Serialize, Deserialize,
)]
#[allow(clippy::large_enum_variant)]
pub enum ProtocolOperation {
    /// Deposit Transaction
    Deposit(DepositInfo),
    /// Deposit Request info
    DepositRequest(DepositRequestInfo),
    /// Checkpoint data
    Checkpoint(SignedBatchCheckpoint),
    // TODO: add other kinds like Proofs and statediffs
}

#[derive(
    Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary, Serialize, Deserialize,
)]
pub struct DepositInfo {
    /// Bitcoin amount
    pub amt: BitcoinAmount,

    /// outpoint
    pub outpoint: OutputRef,

    /// EE address
    pub address: Vec<u8>,
}

#[derive(
    Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary, Serialize, Deserialize,
)]
pub struct DepositRequestInfo {
    /// amount in satoshis
    pub amt: u64,

    /// tapscript control block hash for timelock script
    pub take_back_leaf_hash: [u8; 32],

    /// EE address
    pub address: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct InscriptionData {
    /// payload present in inscription transaction (either batchTx or checkpointTx)
    batch_data: Vec<u8>,
}

impl InscriptionData {
    pub fn new(batch_data: Vec<u8>) -> Self {
        Self { batch_data }
    }

    pub fn batch_data(&self) -> &[u8] {
        &self.batch_data
    }
}
