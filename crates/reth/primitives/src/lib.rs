#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use alloy_sol_types::sol;
use reth_primitives::revm_primitives::FixedBytes;
use serde::{Deserialize, Serialize};

/// Type for withdrawal_intents in rpc.
/// Distinct from `strata_state::bridge_ops::WithdrawalIntent`
/// as this will live in reth repo eventually
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WithdrawalIntent {
    /// Amount to be withdrawn in sats.
    pub amt: u64,

    /// Referenced transaction's txid.
    pub txid: FixedBytes<32>,

    /// Index of referenced output in transaction's vout.
    pub vout: u32,
}

sol! {
    #[allow(missing_docs)]
    event WithdrawalIntentEvent(
        /// Withdrawal amount in sats.
        uint64 amount,
        /// Referenced transaction's txid.
        bytes32 txid,
        /// Index of referenced output in transaction's vout.
        uint32 vout,
    );
}
