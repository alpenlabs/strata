//! Type/traits related to the bridge-related duties.

use express_bridge_tx_builder::prelude::{CooperativeWithdrawalInfo, DepositInfo};
use serde::{Deserialize, Serialize};

/// The various duties that can be assigned to an operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum BridgeDuty {
    /// The duty to create and sign a Deposit Transaction so as to move funds from the user to the
    /// Bridge Address.
    ///
    /// This duty is created when a user deposit request comes in, and applies to all operators.
    SignDeposit(DepositInfo),

    /// The duty to fulfill a withdrawal request that is assigned to a particular operator.
    ///
    /// This duty is created when a user requests a withdrawal by calling a precompile in the EL
    /// and the [`crate::bridge_state::DepositState`] transitions to
    /// [`crate::bridge_state::DepositState::Dispatched`].
    ///
    /// This kicks off the withdrawal process which involves cooperative signing by the operator
    /// set, or a more involved unilateral withdrawal process (in the future) if not all operators
    /// cooperate in the process.
    FulfillWithdrawal(CooperativeWithdrawalInfo),
}

impl From<DepositInfo> for BridgeDuty {
    fn from(value: DepositInfo) -> Self {
        Self::SignDeposit(value)
    }
}

impl From<CooperativeWithdrawalInfo> for BridgeDuty {
    fn from(value: CooperativeWithdrawalInfo) -> Self {
        Self::FulfillWithdrawal(value)
    }
}

/// An alias for a list of bridge duties for readability.
pub type BridgeDuties = Vec<BridgeDuty>;
