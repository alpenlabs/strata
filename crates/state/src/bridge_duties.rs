//! Type/traits related to the bridge-related duties.

use alpen_express_primitives::bridge::OperatorIdx;
use arbitrary::Arbitrary;
use borsh::{BorshDeserialize, BorshSerialize};
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

/// The various states a bridge duty may be in.
///
/// The full state transition looks as follows:
///
/// `Received` --|`CollectingNonces`|--> `CollectedNonces` --|`CollectingPartialSigs`|-->
/// `CollectedSignatures` --|`Broadcasting`|--> `Executed`.
///
/// # Note
///
/// This type does not dictate the exact state transition path. A transition from `Received` to
/// `Executed` is perfectly valid to allow for maximum flexibility.
// TODO: use a typestate pattern with a `next` method that does the state transition. This can
// be left as is to allow for flexible level of granularity. For example, one could just have
// `Received`, `CollectedSignatures` and `Executed`.
#[derive(
    Debug, Clone, PartialEq, Eq, Arbitrary, Serialize, Deserialize, BorshSerialize, BorshDeserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum BridgeDutyStatus {
    /// The duty has been received.
    ///
    /// This usually entails collecting nonces before the corresponding transaction can be
    /// partially signed.
    Received,

    /// The required nonces are being collected.
    CollectingNonces {
        /// The number of nonces collected so far.
        collected: u32,

        /// The indexes of operators that are yet to provide nonces.
        remaining: Vec<OperatorIdx>,
    },

    /// The required nonces have been collected.
    ///
    /// This state can be inferred from the previous state but might still be useful as the
    /// required number of nonces is context-driven and it cannot be determined whether all
    /// nonces have been collected by looking at the above variant alone.
    CollectedNonces,

    /// The partial signatures are being collected.
    CollectingSignatures {
        /// The number of nonces collected so far.
        collected: u32,

        /// The indexes of operators that are yet to provide partial signatures.
        remaining: Vec<OperatorIdx>,
    },

    /// The required partial signatures have been collected.
    ///
    /// This state can be inferred from the previous state but might still be useful as the
    /// required number of signatures is context-driven and it cannot be determined whether all
    /// partial signatures have been collected by looking at the above variant alone.
    CollectedSignatures,

    /// The duty has been executed.
    ///
    /// This means that the required transaction has been fully signed and broadcasted to Bitcoin.
    Executed,
}

impl Default for BridgeDutyStatus {
    fn default() -> Self {
        Self::Received
    }
}

impl BridgeDutyStatus {
    /// Checks if the [`BridgeDutyStatus`] is in its final state.
    pub fn is_done(&self) -> bool {
        matches!(self, BridgeDutyStatus::Executed)
    }
}
