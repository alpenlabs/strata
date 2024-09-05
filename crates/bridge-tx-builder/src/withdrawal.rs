//! Provides types/traits associated with the withdrawal process.

use alpen_express_primitives::l1::XOnlyPk;
use bitcoin::{secp256k1::schnorr::Signature, OutPoint};
use serde::{Deserialize, Serialize};

/// Details for a reimbursement request first produced by the assigned operator and subsequently
/// passed by other operators along with their signature, along with the withdrawal batch. This
/// encapsulates all the information required to create a transaction chain for withdrawal
/// fulfillment. We assume that this reimbursement request has been validated in the rollup full
/// node before being propagated to the bridge client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooperativeWithdrawalInfo {
    deposit_outpoint: OutPoint,
}

impl CooperativeWithdrawalInfo {
    /// Create a new withdrawal request.
    pub fn new(deposit_outpoint: OutPoint) -> Self {
        Self { deposit_outpoint }
    }

    /// The deposit UTXO that is designated to be used for reimbursing the operator that is assigned
    /// to service the withdrawal.
    pub fn deposit_outpoint(&self) -> &OutPoint {
        &self.deposit_outpoint
    }

    /// Construct and sign off on the withdrawal fulfillment transaction based on
    /// information available in the rollup chainstate and the utxo information from the
    /// operator. The `recipient_pubkey` needs to be supplied externally.
    pub fn construct_and_sign(&self, _recipient_pubkey: XOnlyPk) -> Signature {
        unimplemented!();
    }
}

// impl TxKind for `CooperativeWithdrawalInfo`
