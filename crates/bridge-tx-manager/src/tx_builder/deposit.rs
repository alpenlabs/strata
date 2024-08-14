//! Builders related to building deposit-related transactions.
//!
//! Contains types, traits and implementations related to creating various transactions used in the
//! bridge-in dataflow.

use bitcoin::{Amount, OutPoint};
use serde::{Deserialize, Serialize};

use crate::SignatureInfo;

/// The deposit information  required to create the Deposit Transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction UTXO from the user.
    deposit_request_utxo: OutPoint,

    /// The execution layer address to mint the equivalent tokens to.
    /// As of now, this is just the 20-byte EVM address.
    el_address: Vec<u8>,

    /// The amount in bitcoins that the user wishes to deposit.
    amount: Amount,

    /// The metadata associated with the deposit request.
    metadata: DepositMetadata,
}

impl DepositInfo {
    /// Construct the deposit transaction based on some information that depends on the bridge
    /// implementation, the deposit request transaction created by the user and some metadata
    /// related to the rollup.
    pub fn construct_deposit_tx(&self) -> Vec<u8> {
        unimplemented!();
    }
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

/// The details regarding the deposit transaction signing which includes all the information
/// required to create the Deposit Transaction deterministically, as well as the signature if one
/// has already been attached.
///
/// This container encapsulates both the initial duty originating in bitcoin from the user as well
/// as the subsequent signing duty originating from an operator who attaches their signature. Each
/// operator that receives a signature validates, aggregates and stores it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequest {
    /// The details required to create the Deposit Transaction deterministically.
    deposit_info: DepositInfo,

    /// The signature details if the transaction has already been signed by an operator.
    signature_info: Option<SignatureInfo>,
}

impl DepositRequest {
    /// Get the deposit info associated with this request.
    pub fn deposit_info(&self) -> &DepositInfo {
        &self.deposit_info
    }

    /// Get the signature information associated with this request.
    pub fn signature_info(&self) -> &Option<SignatureInfo> {
        &self.signature_info
    }
}
