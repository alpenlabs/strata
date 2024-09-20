use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::buf::Buf32;

/// Information related to relevant transactions to be stored in L1Tx
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub enum ParsedTx {
    /// Deposit Transaction
    Deposit(DepositInfo),
    DepositRequest(DepositRequestInfo),
    RollupInscription(InscriptionData),
    SpentToAddress(Buf32)
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct InscriptionData {
    /// batch_data
    pub batch_data: Vec<u8>,
    /// version
    pub version: u8
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositInfo {
    /// Deposit Amount
    pub amt: u64,
    /// EE Deposit Address
    pub deposit_addr: Vec<u8>
}

#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize, Arbitrary)]
pub struct DepositRequestInfo {
    /// Deposit Amount
    pub amt: u64,
    /// Tapscript Block
    pub control_block: Vec<u8>,
    /// EE Deposit Address
    pub deposit_addr: Vec<u8>
}
