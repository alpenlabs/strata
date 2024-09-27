use alpen_express_primitives::l1::OutputRef;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::batch::SignedBatchCheckpoint;

/// Information related to relevant transactions to be stored in L1Tx
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
#[allow(clippy::large_enum_variant)]

pub enum ProtocolOperation {
    /// Deposit Transaction
    Deposit(DepositInfo),
    DepositRequest(DepositRequestInfo),
    RollupInscription(SignedBatchCheckpoint),
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositInfo {
    /// amount in satoshis
    pub amt: u64,

    /// outpoint
    pub outpoint: OutputRef,

    /// EE address
    pub address: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
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
