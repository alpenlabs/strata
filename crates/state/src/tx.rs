use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use strata_primitives::l1::{BitcoinAmount, OutputRef};

use crate::batch::SignedBatchCheckpoint;

/// Information related to relevant transactions to be stored in L1Tx
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
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

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositInfo {
    /// Bitcoin amount
    pub amt: BitcoinAmount,

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
pub struct InscriptionBlob {
    /// what kind of inscription it is for tagging purpose
    data_type: BlobType,
    /// payload present in inscription transaction (either batchTx or checkpointTx)
    data: Vec<u8>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum BlobType {
    Checkpoint,
    DA,
    Batch
}

impl InscriptionBlob {
    pub fn new(data_type: BlobType, data: Vec<u8>) -> Self {
        Self { data_type, data }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_type(&self) -> BlobType {
        self.data_type
    }
}
