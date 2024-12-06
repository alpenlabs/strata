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
    /// DA data. can be made through `submit_da_blob` RPC.
    DA(Vec<u8>),
    // TODO: add other kinds like statediffs
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

/// Wrapper to hold various types of Inscription data as defined by [`BlobType`] enum.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct InscriptionBlob {
    /// for tagging purpose to understand what kind of inscription it is
    data_type: BlobType,
    /// payload present in inscription transaction (either batchTx or checkpointTx)
    data: Vec<u8>,
}

/// Enum that acts as a tag to separates different types of inscription blobs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
#[borsh(use_discriminant = true)]
pub enum BlobType {
    Checkpoint = 0,
    DA = 1,
}

impl BlobType {
    /// Used to convert Integer to Type mainly for parsing purpose
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Checkpoint),
            1 => Some(Self::DA),
            _ => None,
        }
    }
}

impl InscriptionBlob {
    pub fn new(data_type: BlobType, data: Vec<u8>) -> Self {
        Self { data_type, data }
    }

    /// Raw payload bytes
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Blob Inscription type
    pub fn data_type(&self) -> BlobType {
        self.data_type
    }
}
