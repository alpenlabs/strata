use revm::{ContextStatefulPrecompile, Database};
use revm_primitives::{
    address, Address, Bytes, Log, LogData, PrecompileError, PrecompileErrors, PrecompileOutput,
    PrecompileResult, U256,
};

use crate::primitives::WithdrawalIntentEvent;

// TODO: address?
pub const BRIDGEOUT_ADDRESS: Address = address!("000000000000000000000000000000000b121d9e");
const WITHDRAWAL_WEI: u128 = 10 * (1e18 as u128);

/// Custom precompile to burn rollup native token and add bridge out intent of equal amount.
/// Bridge out intent is created during block payload generation.
/// This precompile validates transaction and burns the bridge out amount.
pub struct BridgeoutPrecompile {
    withdrawal_wei: U256,
}

impl Default for BridgeoutPrecompile {
    fn default() -> Self {
        Self {
            withdrawal_wei: U256::from(WITHDRAWAL_WEI),
        }
    }
}

impl<DB: Database> ContextStatefulPrecompile<DB> for BridgeoutPrecompile {
    fn call(
        &self,
        bytes: &Bytes,
        _gas_limit: u64,
        evmctx: &mut revm::InnerEvmContext<DB>,
    ) -> PrecompileResult {
        // calldata must be 32bytes x-only pubkey
        if bytes.len() != 32 {
            return Err(PrecompileError::other("invalid data").into());
        }

        // bridgeout MUST be for exactly 10BTC
        let value = evmctx.env.tx.value;
        if value != self.withdrawal_wei {
            return Err(PrecompileError::other("invalid withdrawal value").into());
        }

        let (sats, _) = value.div_rem(U256::from(10_000_000_000u128));

        let Ok(amount) = sats.try_into() else {
            // should never happen
            return Err(PrecompileErrors::Fatal {
                msg: "above max withdrawal amt".into(),
            });
        };

        // log bridge withdrawal intent
        let evt = WithdrawalIntentEvent {
            amount,
            dest_pk: bytes.clone(),
        };
        let logdata = LogData::from(&evt);

        evmctx.journaled_state.log(Log {
            address: BRIDGEOUT_ADDRESS,
            data: logdata,
        });

        // burn value sent to bridge
        let Ok((account, _)) = evmctx.load_account(BRIDGEOUT_ADDRESS) else {
            // should never happen
            return Err(PrecompileErrors::Fatal {
                msg: "could not load account".into(),
            });
        };

        let (new_balance, overflow) = account.info.balance.overflowing_sub(value);
        if overflow {
            // should never happen
            return Err(PrecompileErrors::Fatal {
                msg: "invalid balance".into(),
            });
        }

        account.info.balance = new_balance;

        // TODO: gas for bridge out, using 0 gas currently
        Ok(PrecompileOutput::new(0, Bytes::new()))
    }
}
