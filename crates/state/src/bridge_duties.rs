//! Type/traits related to the bridge-related duties.

use express_bridge_txm::{DepositRequest, ReimbursementRequest};
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
    SignDeposit(DepositRequest),

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
    SignWithdrawal(ReimbursementRequest),
}

/// A container for bridge duties after `from_height` till `to_height` blocks in the rollup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDuties {
    /// The bridge duties computed from the chainstate and the bridge p2p message queue.
    duties: Vec<Duty>,

    /// The rollup block height after which the duties are computed.
    from_height: u64,

    /// The rollup block height till which the duties are computed.
    to_height: u64,
}
