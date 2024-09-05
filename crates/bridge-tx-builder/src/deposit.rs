//! Builders related to building deposit-related transactions.
//!
//! Contains types, traits and implementations related to creating various transactions used in the
//! bridge-in dataflow.

use alpen_express_primitives::l1::BitcoinAddress;
use bitcoin::{Amount, OutPoint, TapNodeHash};
use serde::{Deserialize, Serialize};

/// The deposit information  required to create the Deposit Transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction outpoints from the users.
    deposit_request_outpoint: OutPoint,

    /// The execution layer address to mint the equivalent tokens to.
    /// As of now, this is just the 20-byte EVM address.
    el_address: Vec<u8>,

    /// The amount in bitcoins that the user wishes to deposit.
    total_amount: Amount,

    /// The hash of the take back leaf in the Deposit Request Transaction (DRT) as provided by the
    /// user in their `OP_RETURN` output.
    take_back_leaf_hash: TapNodeHash,

    /// The original taproot address in the Deposit Request Transaction (DRT) output used to
    /// validate computation internally.
    original_taproot_addr: BitcoinAddress,
}

impl DepositInfo {
    /// Create a new deposit info with all the necessary data required to create a deposit
    /// transaction.
    pub fn new(
        deposit_request_outpoint: OutPoint,
        el_address: Vec<u8>,
        total_amount: Amount,
        take_back_leaf_hash: TapNodeHash,
        original_taproot_addr: BitcoinAddress,
    ) -> Self {
        Self {
            deposit_request_outpoint,
            el_address,
            total_amount,
            take_back_leaf_hash,
            original_taproot_addr,
        }
    }

    /// Get the total deposit amount that needs to be bridged-in.
    pub fn total_amount(&self) -> &Amount {
        &self.total_amount
    }

    /// Get the address in EL to mint tokens to.
    pub fn el_address(&self) -> &[u8] {
        &self.el_address
    }

    /// Get the outpoint of the Deposit Request Transaction (DRT) that is to spent in the Deposit
    /// Transaction (DT).
    pub fn deposit_request_outpoint(&self) -> &OutPoint {
        &self.deposit_request_outpoint
    }

    /// Get the hash of the user-takes-back leaf in the taproot of the Deposit Request Transaction
    /// (DRT).
    pub fn take_back_leaf_hash(&self) -> &TapNodeHash {
        &self.take_back_leaf_hash
    }
}

// TODO: impl `TxKind` on `DepositInfo` (WIP in EXP-130).
