use alloy_rpc_types::Withdrawal as AlloyWithdrawal;
use reth_primitives::Withdrawal as RethWithdrawal;
/// A trait to convert from Alloy types to Reth types.
pub trait IntoReth<T> {
    fn into_reth(self) -> T;
}

impl IntoReth<RethWithdrawal> for AlloyWithdrawal {
    fn into_reth(self) -> RethWithdrawal {
        RethWithdrawal {
            index: self.index,
            validator_index: self.validator_index,
            amount: self.amount,
            address: self.address,
        }
    }
}
