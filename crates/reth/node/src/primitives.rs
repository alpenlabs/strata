use alloy_sol_types::sol;
use reth_primitives::B512;
use serde::{Deserialize, Serialize};

/// Type for withdrawal_intents in rpc.
/// Distinct from [`bridge_ops::WithdrawalIntents`] as this will live in reth repo eventually
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WithdrawalIntent {
    /// Amount of currency to be withdrawn.
    pub amt: u64,

    /// Destination public key for the withdrawal
    pub dest_pk: B512,
}

sol! {
    #[allow(missing_docs)]
    event WithdrawalIntentEvent(
        uint64 amount,
        bytes dest_pk,
    );
}
