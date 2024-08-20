//! Builders related to building deposit-related transactions.
//!
//! Contains types, traits and implementations related to creating various transactions used in the
//! bridge-in dataflow.

use bitcoin::{secp256k1::schnorr::Signature, Amount, OutPoint, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use crate::{Signed, Unsigned};

/// The deposit information  required to create the Deposit Transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositInfo {
    /// The deposit request transaction outpoint from the user.
    deposit_request_outpoint: OutPoint,

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
pub struct DepositRequest<SigStatus = Unsigned> {
    /// The details required to create the Deposit Transaction deterministically.
    deposit_info: DepositInfo,

    /// The signature details.
    signature_info: SigStatus,
}

impl DepositRequest {
    /// Get the deposit info associated with this request.
    pub fn deposit_info(&self) -> &DepositInfo {
        &self.deposit_info
    }

    /// Sign the transaction.
    pub fn add_signature(&self) -> DepositRequest<Signed> {
        unimplemented!()
    }
}

impl DepositRequest<Signed> {
    /// Get the signature in the signed deposit request.
    pub fn signature(&self) -> &Signature {
        self.signature_info.inner().signature()
    }

    /// Get the signer pubkey in the signed deposit request.
    pub fn signer_pubkey(&self) -> &XOnlyPublicKey {
        self.signature_info.inner().signer_pubkey()
    }
}
