use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use strata_primitives::l1::{BitcoinAmount, OutputRef};

use crate::batch::SignedBatchCheckpoint;

/// Information related to relevant transactions to be stored in an `L1Tx`.
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
    // TODO: add other kinds like proofs and state diffs
}

/// Similar to [`ProtocolOperation`] except that this also contains blob data which is not relevant
/// to chain.
#[allow(clippy::large_enum_variant)]
pub enum RawProtocolOperation {
    /// Deposit Transaction
    Deposit(DepositInfo),
    /// Deposit Request info
    DepositRequest(DepositRequestInfo),
    /// Checkpoint data
    Checkpoint(SignedBatchCheckpoint),
    // TODO: add other kinds like proofs and state diffs
}

impl From<RawProtocolOperation> for ProtocolOperation {
    fn from(val: RawProtocolOperation) -> Self {
        match val {
            RawProtocolOperation::DepositRequest(d) => ProtocolOperation::DepositRequest(d),
            RawProtocolOperation::Deposit(d) => ProtocolOperation::Deposit(d),
            RawProtocolOperation::Checkpoint(c) => ProtocolOperation::Checkpoint(c),
        }
    }
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
