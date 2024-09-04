//! Provides types/traits associated with the withdrawal process.

use alpen_express_primitives::l1::BitcoinAddress;
use bitcoin::{secp256k1::schnorr::Signature, Amount, OutPoint};
use serde::{Deserialize, Serialize};

use crate::SignatureInfo;

// NOTE: The following structs represent the various states a `Withdrawal` can be in.
// An extra struct to represent the `FullySigned` state is not necessary as bitcoin's logic
// prohibits such transactions from being confirmed. However, we do need a `Validated` state since
// this validation is based on the rollup logic which the bitcoin consensus is not privy to and it
// can be detrimental to sign/publish transactions that are not valid in the rollup context (for
// example, a withdrawal that is not assigned to the claimant).

/// Details for a reimbursement request first produced by the assigned operator and subsequently
/// passed by other operators along with their signature, along with the withdrawal batch. This
/// encapsulates all the information required to create a transaction chain for withdrawal
/// fulfillment. We assume that this reimbursement request has been validated in the rollup full
/// node before being propagated to the bridge client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementRequest {
    operator_reserved_outpoint: OutPoint,
    deposit_outpoint: OutPoint,
    signature_info: Option<SignatureInfo>,
}

impl ReimbursementRequest {
    /// Create a new withdrawal request.
    pub fn new(
        operator_reserved_outpoint: OutPoint,
        deposit_outpoint: OutPoint,
        signature_info: Option<SignatureInfo>,
    ) -> Self {
        Self {
            operator_reserved_outpoint,
            deposit_outpoint,
            signature_info,
        }
    }

    /// Get the information required to create the withdrawal
    pub fn operator_reserved_outpoint(&self) -> &OutPoint {
        &self.operator_reserved_outpoint
    }

    /// Get the signature information.
    pub fn signature_info(&self) -> &Option<SignatureInfo> {
        &self.signature_info
    }

    /// The deposit UTXO that is designated to be used for reimbursing the operator that is assigned
    /// to service the withdrawal.
    pub fn deposit_outpoint(&self) -> &OutPoint {
        &self.deposit_outpoint
    }

    /// Construct and sign off on the withdrawal reimbursement transaction based on
    /// information available in the rollup chainstate and the utxo information from the
    /// operator. The requests_table needs to be supplied externally.
    // NOTE: `requests_table` is equivalent to `Vec<WithdrawalIntent>` but having that adds a
    // dependency on `alpen-express-state` for this crate which results in a cyclic dependency.
    // It is best to keep this crate which is responsible for creating transactions and signing
    // them as independent of other `express` crates as possible.
    pub fn construct_and_sign(&self, _requests_table: Vec<(BitcoinAddress, Amount)>) -> Signature {
        unimplemented!();
    }
}
