//! Provides types/traits associated with the withdrawal process.

use alpen_express_state::bridge_state::OperatorIdx;
use bitcoin::{secp256k1::schnorr::Signature, OutPoint};
use serde::{Deserialize, Serialize};

/// A marker type indicating that a withdrawal request has come in and needs to be validated.
#[derive(Debug, Clone)]
pub struct Requested;

/// A marker type indicating that a withdrawal has been validated and can be signed.
#[derive(Debug, Clone)]
pub struct Validated;

/// The withdrawal information required in the cooperative path to gather signatures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalInfo<State = Requested> {
    /// The UTXO in the operator's reserved address that they want to use to fulfill a withdrawal
    /// request.
    operator_reserved_utxo: OutPoint,

    /// The Deposit UTXO in the Bridge Address that is to be used to reimburse the operator.
    deposit_utxo: OutPoint,

    /// The ID of the operator requesting the signature.
    operator_id: OperatorIdx,

    state: std::marker::PhantomData<State>,
    // Rollup data required to verify withdrawal assignment.
    // TODO: This possibly requires SSZ impl for efficient verification
    // rollup_block_info: ??
}

impl WithdrawalInfo<Requested> {
    /// Validate that the operator requesting
    pub fn validate(&self) -> WithdrawalInfo<Validated> {
        unimplemented!();
    }
}

impl WithdrawalInfo<Validated> {
    /// Sign off on the validated withdrawal transaction
    pub fn sign_reimbursement(&self) -> Signature {
        unimplemented!();
    }
}
