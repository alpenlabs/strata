use alloy_sol_types::sol;
use reth_primitives::B256;
use serde::{Deserialize, Serialize};

/// Type for withdrawal_intents in rpc.
/// Distinct from [`bridge_ops::WithdrawalIntents`] as this will live in reth repo eventually
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
        bytes dest_pk,
    );
}
