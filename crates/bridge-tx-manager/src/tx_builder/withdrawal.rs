//! Provides types/traits associated with the withdrawal process.

use alpen_express_primitives::l1::BitcoinAddress;
use async_trait::async_trait;
use bitcoin::{secp256k1::schnorr::Signature, Amount, OutPoint};
use serde::{Deserialize, Serialize};

use crate::SignatureInfo;

// NOTE: The following structs represent the various states a `Withdrawal` can be in.
// An extra struct to represent the `FullySigned` state is not necessary as bitcoin's logic
// prohibits such transactions from being confirmed. However, we do need a `Validated` state since
// this validation is based on the rollup logic which the bitcoin consensus is not privy to and it
// can be detrimental to sign/publish transactions that are not valid in the rollup context (for
// example, a withdrawal that is not assigned to the claimant).

/// The state of a withdrawal reimbursement request having been just created.
#[derive(Debug, Clone)]
pub struct Requested;

/// The state of a withdrawal reimbusement request having been validated.
#[derive(Debug, Clone)]
pub struct Validated;

/// Details for a reimbursement request first produced by the assigned operator and subsequently
/// passed by other operators along with their signature, along with the withdrawal batch. This
/// encapsulates all the information required to create a transaction chain for withdrawal
/// fulfillment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementRequest<Status = Requested> {
    operator_reserved_utxo: OutPoint,
    deposit_utxo: OutPoint,
    signature_info: Option<SignatureInfo>,

    state: std::marker::PhantomData<Status>,
}

impl ReimbursementRequest {
    /// Create a new withdrawal request.
    pub fn new(
        operator_reserved_utxo: OutPoint,
        deposit_utxo: OutPoint,
        signature_info: Option<SignatureInfo>,
    ) -> Self {
        Self {
            operator_reserved_utxo,
            deposit_utxo,
            signature_info,

            state: std::marker::PhantomData,
        }
    }

    /// Get the information required to create the withdrawal
    pub fn operator_reserved_utxo(&self) -> &OutPoint {
        &self.operator_reserved_utxo
    }

    /// Get the signature information.
    pub fn signature_info(&self) -> &Option<SignatureInfo> {
        &self.signature_info
    }

    /// The deposit UTXO that is designated to be used for reimbursing the operator that is assigned
    /// to service the withdrawal.
    pub fn deposit_utxo(&self) -> &OutPoint {
        &self.deposit_utxo
    }

    /// Validate the request against some implementation of the [`ValidateWithdrawal`].
    ///
    /// The validator may have more context (such as access to a database or an RPC client) to
    /// perform validation.
    pub fn validate_request<T: ValidateWithdrawal>(
        &self,
        _validator: &T,
    ) -> Option<ReimbursementRequest<Validated>> {
        // TODO: this should really return a `Result<T>` but
        // that requires an error type that the bridge
        // client understands. Could replace with `anyhow::<T>`
        unimplemented!()
    }
}

impl ReimbursementRequest<Validated> {
    /// Construct and sign off on the validated withdrawal reimbursement transaction based on
    /// information available in the rollup chainstate and the utxo information from the
    /// operator. The requests_table needs to be supplied externally.
    // NOTE: `requests_table` is equivalent to `Vec<WithdrawalBatch>` (without the block window
    // information) but having that adds a dependency on `alpen-express-state` for this crate which
    // results in a cyclic dependency. It is best to keep this crate which is responsible for
    // creating transactions and signing them as independent of other `express` crates as possible.
    pub fn construct_and_sign(&self, _requests_table: Vec<(BitcoinAddress, Amount)>) -> Signature {
        unimplemented!();
    }
}

/// Trait to define the ability to validate a reimbursement request.
///
/// This can be implemented at the call site where additional resources may be available (such as an
/// RPC client) to perform the validation.
#[async_trait]
pub trait ValidateWithdrawal {
    /// Validate the withdrawal info based on some context.
    async fn validate_withdrawal(&self, reimbursement_req: &ReimbursementRequest) -> bool;
}
