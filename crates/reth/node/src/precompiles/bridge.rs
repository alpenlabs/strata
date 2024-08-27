use revm::{ContextStatefulPrecompile, Database};
use revm_primitives::{
    address, Address, Bytes, Log, LogData, PrecompileError, PrecompileErrors, PrecompileOutput,
    PrecompileResult, U256,
};

use crate::primitives::WithdrawalIntentEvent;

// TODO: address?
pub const BRIDGEOUT_ADDRESS: Address = address!("000000000000000000000000000000000b121d9e");
const MIN_WITHDRAWAL_WEI: u128 = 1_000_000_000_000_000_000u128;

/// Custom precompile to burn rollup native token and add bridge out intent of equal amount.
/// Bridge out intent is created during block payload generation.
/// This precompile validates transaction and burns the bridge out amount.
pub struct BridgeoutPrecompile {
    min_withdrawal_wei: U256,
}

impl Default for BridgeoutPrecompile {
    fn default() -> Self {
        Self {
            min_withdrawal_wei: U256::from(MIN_WITHDRAWAL_WEI),
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
        // ensure valid calldata
        if bytes.len() != 64 {
            return Err(PrecompileErrors::Error(PrecompileError::other(
                "invalid data",
            )));
        }

        // ensure minimum bridgeout amount
        let value = evmctx.env.tx.value;
        if value < self.min_withdrawal_wei {
            return Err(PrecompileErrors::Error(PrecompileError::other(
                "below min withdrawal amt",
            )));
        }

        let (sats, rem) = value.div_rem(U256::from(10_000_000_000u128));

        if !rem.is_zero() {
            // ensure there are no leftovers that get lost.
            // is this important?
            return Err(PrecompileErrors::Error(PrecompileError::other(
                "value must be exact sats",
            )));
        }

        let Ok(amount) = sats.try_into() else {
            // should never happen. 2^64 ~ 8700 x total_btc_stats
            return Err(PrecompileErrors::Error(PrecompileError::other(
                "above max withdrawal amt",
            )));
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

        // TODO: burn value

        // TODO: gas for bridge out, using 0 gas currently
        Ok(PrecompileOutput::new(0, Bytes::new()))
    }
}
