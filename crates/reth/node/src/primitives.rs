use alloy_sol_types::sol;
use reth_primitives::B512;
use serde::{Deserialize, Serialize};

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
