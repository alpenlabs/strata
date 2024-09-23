use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};


/// Information related to relevant transactions to be stored in L1Tx
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum RelevantTxInfo {
    /// Deposit Transaction
    Deposit(DepositInfo),
    DepositRequest(DepositReqeustInfo),
    RollupInscription(InscriptionData),
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
    pub tap_ctrl_blk_hash: [u8;32],

    /// EE address
    pub address: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct InscriptionData {
    /// payload present in inscription transaction (either batchTx or checkpointTx)
    batch_data: Vec<u8>,
}

impl InscriptionData {
    pub const ROLLUP_NAME_TAG: &[u8] = &[1];
    pub const VERSION_TAG: &[u8] = &[2];
    pub const BATCH_DATA_TAG: &[u8] = &[3];

    pub fn new(batch_data: Vec<u8>) -> Self {
        Self {
            batch_data,
        }
    }

    pub fn batch_data(&self) -> &[u8] {
        &self.batch_data
    }

}
