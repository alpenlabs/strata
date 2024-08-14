//! Types for managing pending bridging operations in the CL state.

use alpen_express_primitives::{
    buf::Buf64,
    l1::{BitcoinAmount, OutputRef},
    prelude::BitcoinAddress,
};
use arbitrary::Arbitrary;
use bitcoin::OutPoint;
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use crate::bridge_state::OperatorEntry;

pub const WITHDRAWAL_DENOMINATION: BitcoinAmount = BitcoinAmount::from_int_btc(10);

pub type BitcoinBlockHeight = u64;

/// Describes an intent to withdraw that hasn't been dispatched yet.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct WithdrawalIntent {
    /// Quantity of L1 asset, for Bitcoin this is sats.
    amt: BitcoinAmount,

    /// Destination address.
    dest_pk: BitcoinAddress,
}

impl WithdrawalIntent {
    pub fn new(amt: u64, dest_pk: Buf64) -> Self {
        Self { amt, dest_pk }
    }

    pub fn into_parts(&self) -> (u64, Buf64) {
        (self.amt, self.dest_pk)
    }
}

/// Set of withdrawals that are assigned to a deposit bridge utxo.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct WithdrawalBatch {
    /// A series of [WithdrawalIntent]'s who sum does not exceed [`WITHDRAWAL_DENOMINATION`].
    intents: Vec<WithdrawalIntent>,

    /// The operator that is assigned the withdrawal.
    /// This happens when the sum of the intents equals [`WITHDRAWAL_DENOMINATION`]
    assignee: Option<OperatorEntry>,

    /// The particular deposit UTXO that is to be used to service the current batch.
    ///
    /// The deposit UTXO is predetermined when the withdrawal batch is first created as all
    /// such UTXOs are functionally indistinguishable.
    deposit_utxo: OutputRef,

    /// The bitcoin block height before which the withdrawal must be completed.
    /// When set to 0, it means that the withdrawal cannot be processed yet.
    valid_till_blockheight: BitcoinBlockHeight,
}

impl WithdrawalBatch {
    /// Gets the total value of the batch.  This must be less than the size of
    /// the utxo it's assigned to.
    pub fn get_total_value(&self) -> BitcoinAmount {
        self.intents.iter().map(|wi| wi.amt).sum()
    }

    /// Adds an intent to the batch and returns the assigned operator if the total withdrawal amount
    /// equals [`WITHDRAWAL_DENOMINATION`].
    fn add_and_assign(&self, _withdrawal_intent: &WithdrawalIntent) -> Option<OperatorEntry> {
        unimplemented!();
    }

    pub fn intents(&self) -> &[WithdrawalIntent] {
        &self.intents[..]
    }

    pub fn assignee(&self) -> &Option<OperatorEntry> {
        &self.assignee
    }

    pub fn valid_till_blockheight(&self) -> &BitcoinBlockHeight {
        &self.valid_till_blockheight
    }

    pub fn deposit_utxo(&self) -> &OutPoint {
        self.deposit_utxo.outpoint()
    }
}

/// Describes a deposit data to be processed by an EE.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DepositIntent {
    /// Quantity in the L1 asset, for Bitcoin this is sats.
    amt: BitcoinAmount,

    /// Description of the encoded address. For Ethereum this is the 20-byte
    /// address.
    dest_ident: Vec<u8>,
}
