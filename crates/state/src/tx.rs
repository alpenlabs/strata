use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::batch::SignedBatchCheckpoint;

/// Information related to relevant transactions to be stored in L1Tx
#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, Arbitrary, PartialEq, Eq)]
pub enum ProtocolOperation {
    /// Deposit Transaction
    Deposit(DepositInfo),
    DepositRequest(DepositReqeustInfo),
    RollupInscription(SignedBatchCheckpoint),
    SpentToAddress(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositInfo {
    /// amount in satoshis
    pub amt: u64,

    /// outpoint where amount is present
    pub deposit_outpoint: u16,

    /// EE address
    pub address: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositReqeustInfo {
    /// amount in satoshis
    pub amt: u64,

    /// tapscript control block hash for timelock script
    pub tap_ctrl_blk_hash: [u8; 32],

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
