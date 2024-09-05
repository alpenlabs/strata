//! Type/traits related to the bridge-related duties.

use express_bridge_tx_builder::prelude::DepositInfo;
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
}

/// A container for bridge duties based on the state in rollup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeDuties(Vec<Duty>);
