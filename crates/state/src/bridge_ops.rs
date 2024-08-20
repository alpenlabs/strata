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

/// Describes an intent to withdraw that hasn't been dispatched yet.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct WithdrawalIntent {
    /// Quantity of L1 asset, for Bitcoin this is sats.
    amt: BitcoinAmount,

    /// Destination address.
    dest_pk: BitcoinAddress,
}

impl WithdrawalIntent {
    pub fn new(amt: BitcoinAmount, dest_pk: BitcoinAddress) -> Self {
        Self { amt, dest_pk }
    }

    pub fn into_parts(&self) -> (BitcoinAmount, BitcoinAddress) {
        (self.amt, self.dest_pk.clone())
    }

    pub fn amt(&self) -> &BitcoinAmount {
        &self.amt
    }

    pub fn dest_pk(&self) -> &BitcoinAddress {
        &self.dest_pk
    }
}

/// Set of withdrawals that are assigned to a deposit bridge utxo.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
pub struct WithdrawalBatch {
    /// A series of [WithdrawalIntent]'s who sum does not exceed [`WITHDRAWAL_DENOMINATION`].
    intents: Vec<WithdrawalIntent>,
}

impl WithdrawalBatch {
    /// Gets the total value of the batch.  This must be less than the size of
    /// the utxo it's assigned to.
    pub fn get_total_value(&self) -> BitcoinAmount {
        self.intents.iter().map(|wi| wi.amt).sum()
    }

    pub fn intents(&self) -> &[WithdrawalIntent] {
        &self.intents[..]
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
