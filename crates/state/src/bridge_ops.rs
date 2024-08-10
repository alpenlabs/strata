//! Types for managing pending bridging operations in the CL state.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

use alpen_express_primitives::{l1::BitcoinAmount, prelude::BitcoinAddress};

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
    pub fn into_parts(&self) -> (u64, Buf64) {
        (self.amt, self.dest_pk)
    }
}

/// Set of withdrawals that are assigned to a deposit bridge utxo.
#[derive(
    Default, Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize, Serialize, Deserialize,
)]
pub struct WithdrawalBatch {
    /// A series of [WithdrawalIntent]'s who sum does not exceed [`WITHDRAWAL_DENOMINATION`].
    intents: Vec<WithdrawalIntent>,

    /// The operator that is assigned the withdrawal.
    /// This happens when the sum of the intents equals [`WITHDRAWAL_DENOMINATION`]
    assignee: Option<OperatorEntry>,
}

impl WithdrawalBatch {
    /// Gets the total value of the batch.  This must be less than the size of
    /// the utxo it's assigned to.
    pub fn get_total_value(&self) -> u64 {
        self.intents.iter().map(|wi| wi.amt.to_sat()).sum()
    }

    /// Adds an intent to the batch and returns the assigned operator if the total withdrawal amount
    /// equals [`WITHDRAWAL_DENOMINATION`].
    fn add_and_assign(&self, _withdrawal_intent: &WithdrawalIntent) -> Option<OperatorEntry> {
        unimplemented!();
    }
}

/// Describes a deposit data to be processed by an EE.
#[derive(Clone, Debug, Eq, PartialEq, BorshDeserialize, BorshSerialize)]
pub struct DepositIntent {
    /// Quantity in the L1 asset, for Bitcoin this is sats.
    amt: BitcoinAmount,

    /// Description of the encoded address.  For Ethereum this is the 20-byte
    /// address.
    dest_ident: Vec<u8>,
}
