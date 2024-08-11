//! Provides types/traits associated with the withdrawal process.

use bitcoin::{secp256k1::schnorr::Signature, OutPoint};
use serde::{Deserialize, Serialize};

/// A trait to define the ability to construct a Withdrawal Reimbursement Transaction.
pub trait ConstructReimbursementTx: Clone + Sized {
    /// Construct the Withdrawal Reimbursement Transaction based on the withdrawal information
    /// gathered from the rollup, and the specific reserved UTXO provided by an operator.
    fn construct_reimbursement_tx(&self) -> Vec<u8>;
    // TODO: add more methods required to construct the final reimbursement transaction.
}

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

    state: std::marker::PhantomData<State>,
    // Rollup data required to verify withdrawal assignment.
    // TODO: This possibly requires SSZ impl for efficient verification
    // An alternative is to have an RPC on the full node that allows querying the assignee
    // information from the chainstate via the `deposit_utxo` (as each such utxo is mapped to at
    // most one unique assignee at any given time).
    // rollup_block_info: ??
}

impl WithdrawalInfo<Requested> {
    /// Validate that the operator requesting
    pub fn validate(&self) -> WithdrawalInfo<Validated> {
        unimplemented!();
    }
}

impl ConstructReimbursementTx for WithdrawalInfo<Validated> {
    fn construct_reimbursement_tx(&self) -> Vec<u8> {
        unimplemented!();
    }
}

impl WithdrawalInfo<Validated> {
    /// Sign off on the validated withdrawal transaction
    pub fn sign_reimbursement(&self) -> Signature {
        unimplemented!();
    }
}
