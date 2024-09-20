#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use alloy_sol_types::sol;
use reth_primitives::B256;
use serde::{Deserialize, Serialize};

/// Type for withdrawal_intents in rpc.
/// Distinct from `alpen_express_state::bridge_ops::WithdrawalIntent`
/// as this will live in reth repo eventually
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WithdrawalIntent {
    /// Amount to be withdrawn in sats.
    pub amt: u64,

    /// Destination public key for the withdrawal
    pub dest_pk: B256,
}

sol! {
    #[allow(missing_docs)]
    event WithdrawalIntentEvent(
        /// Withdrawal amount in sats
        uint64 amount,
        /// 32 bytes pubkey for withdrawal address in L1
        bytes32 dest_pk,
    );
}
