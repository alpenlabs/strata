//! Type/traits related to the bridge-related duties.

use express_bridge_txm::{DepositInfo, ReimbursementRequest};
use serde::{Deserialize, Serialize};

use crate::bridge_ops::WithdrawalBatch;

/// The various duties that can be assigned to an operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Duty {
    /// The duty to create and sign a Deposit Transaction so as to move funds from the user to the
    /// Bridge Address.
    ///
    /// This duty is created when a user deposit request comes in, and applies to all operators.
    SignDeposit(DepositInfo),

    /// The duty to fulfill a withdrawal request that is assigned to a particular operator.
    ///
    /// This duty is created when a user submits a withdrawal request, and only applies to the
    /// operator that is assigned the [`WithdrawalBatch`].
    ///
    /// As each deposit UTXO is uniquely assigned to an operator during withdrawals, this UTXO can
    /// be used to query for the complete [`WithdrawalBatch`] information. We are not sending the
    /// [`WithdrawalBatch`] out directly as
    FulfillWithdrawal(WithdrawalBatch),

    /// The duty to sign the Withdrawal Reimbursement Transaction.
    ///
    /// This duty is created by the operator that is assigned the [`WithdrawalBatch`], and applies
    /// to the rest of the operators.
    // TODO: move this to a `BridgeMessage` scope after <https://alpenlabs.atlassian.net/browse/EXP-108>.
    SignWithdrawal(ReimbursementRequest),

    /// Other messages originating from the bridge clients. This encapsulates the `BridgeMessage`
    /// as will be defined in <https://alpenlabs.atlassian.net/browse/EXP-108>.
    P2PMessage,
}

/// A container for bridge duties after `from_height` till `to_height` blocks in the rollup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDuties {
    /// The bridge duties computed from the chainstate and the bridge p2p message queue.
    duties: Vec<Duty>,

    /// The checkpoint in the CL after which the duties are computed.
    checkpoint_start: u64,

    /// The checkpoint in the CL till which the duties are computed (inclusive).
    checkpoint_end: u64,
}
