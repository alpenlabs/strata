//! Types for managing pending bridging operations in the CL state.

use alpen_express_primitives::buf::Buf64;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};

/// Describes an intent to withdraw that hasn't been dispatched yet.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct WithdrawalIntent {
    /// Quantity of L1 asset, for Bitcoin this is sats.
    amt: u64,

    /// Dest taproot pubkey.
    // TODO this is somewhat of a placeholder, we might make it more general or
    // wrap it better
    dest_pk: Buf64,
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
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct WithdrawalBatch {
    intents: Vec<WithdrawalIntent>,
}

impl WithdrawalBatch {
    /// Gets the total value of the batch.  This must be less than the size of
    /// the utxo it's assigned to.
    pub fn get_total_value(&self) -> u64 {
        self.intents.iter().map(|wi| wi.amt).sum()
    }
}

/// Describes a deposit data to be processed by an EE.
#[derive(Clone, Debug, Eq, PartialEq, Arbitrary, BorshDeserialize, BorshSerialize)]
pub struct DepositIntent {
    /// Quantity in the L1 asset, for Bitcoin this is sats.
    amt: u64,

    /// Description of the encoded address.  For Ethereum this is the 20-byte
    /// address.
    dest_ident: Vec<u8>,
}
