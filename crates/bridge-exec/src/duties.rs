//! This module defines the duties and relevant traits that the bridge client cares about.
use alpen_express_state::bridge_ops::WithdrawalBatch;
use express_bridge_txm::{DepositInfo, SignatureInfo, WithdrawalInfo};
use serde::{Deserialize, Serialize};

/// The various duties that can be assigned to an operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Duty {
    /// The duty to create and sign a Deposit Transaction so as to move funds from the user to the
    /// Bridge Address.
    ///
    /// This duty is created when a user deposit request comes in, and applies to all operators.
    SignDeposit(DepositRequest),

    /// The duty to fulfill a withdrawal request that is assigned to a particular operator.
    ///
    /// This duty is created when a user submits a withdrawal request, and only applies to the
    /// operator that is assigned the [`WithdrawalBatch`].
    FulfillWithdrawal(WithdrawalBatch),

    /// The duty to sign the Withdrawal Reimbursement Transaction.
    ///
    /// This duty is created by the operator that is assigned the [`WithdrawalBatch`], and applies
    /// to the rest of the operators.
    SignWithdrawal(ReimbursementRequest),
}

/// The details regarding the deposit transaction signing which includes all the information
/// required to create the Deposit Transaction deterministically, as well as the signature if one
/// has already been attached.
///
/// This container encapsulates both the initial duty originating in bitcoin from the user as well
/// as the subsequent signing duty originiating from an operator who attaches their signature. Each
/// operator that receives a foreign signature validates, aggregates and stores it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRequest {
    /// The details required to create the Deposit Transaction deterministically.
    deposit_info: DepositInfo,

    /// The signature details if the transaction has already been signed by an operator.
    signature_info: Option<SignatureInfo>,
}

/// Details for a reimbursement request first produced by the assigned operator and subsequently
/// passed by other operators along with their signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementRequest {
    withdrawal_info: WithdrawalInfo,
    signature_info: Option<SignatureInfo>,
}
