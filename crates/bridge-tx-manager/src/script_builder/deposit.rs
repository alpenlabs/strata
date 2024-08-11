//! Builders related to building deposit-related transactions.
//!
//! Contains types, traits and implementations related to creating various transactions used in the
//! bridge-in dataflow.

use bitcoin::{Amount, OutPoint};
use reth_primitives::Address as RollupAddress;
use serde::{Deserialize, Serialize};

/// A trait to define the ability to construct a deposit transaction.
pub trait ConstructDepositTx: Clone + Sized {
    /// Construct the deposit transaction based on some information that depends on the bridge
    /// implementation, the deposit request transaction created by the user and some metadata
    /// related to the rollup.
    fn construct_deposit_tx(&self) -> Vec<u8>;
    // TODO: add more methods required to construct the Deposit Transaction.
}

/// The metadata associated with a deposit. This will be used to communicated additional
/// information to the rollup. For now, this only carries limited information but we may extend
/// it later.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositMetadata {
    /// The protocol version that the deposit is associated with.
    version: String,

    /// Special identifier that helps the `alpen-exrpress-btcio::reader` worker identify relevant
    /// deposits.
    // TODO: Convert this to an enum that handles various identifiers if necessary in the future.
    // For now, this identifier will be a constant.
    identifier: String,
}

/// The deposit information  required to create the Deposit Transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction UTXO from the user.
    deposit_request_utxo: OutPoint,

    /// The rollup address to mint the equivalent tokens to.
    rollup_address: RollupAddress,

    /// The amount in bitcoins that the user wishes to deposit.
    amount: Amount,

    /// The metadata associated with the deposit request.
    metadata: DepositMetadata,
}

impl ConstructDepositTx for DepositInfo {
    fn construct_deposit_tx(&self) -> Vec<u8> {
        unimplemented!();
    }
}
