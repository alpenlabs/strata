use std::array::TryFromSliceError;

use revm::{ContextStatefulPrecompile, Database};
use revm_primitives::{
    Bytes, FixedBytes, Log, LogData, PrecompileError, PrecompileErrors, PrecompileOutput,
    PrecompileResult, U256,
};
use strata_reth_primitives::WithdrawalIntentEvent;

pub use crate::constants::BRIDGEOUT_ADDRESS;
use crate::utils::wei_to_sats;

/// Ensure that input is exactly 32 bytes
fn try_into_pubkey(maybe_pubkey: &Bytes) -> Result<FixedBytes<32>, TryFromSliceError> {
    maybe_pubkey.as_ref().try_into()
}

/// Custom precompile to burn rollup native token and add bridge out intent of equal amount.
/// Bridge out intent is created during block payload generation.
/// This precompile validates transaction and burns the bridge out amount.
pub struct BridgeoutPrecompile {
    fixed_withdrawal_wei: U256,
}

impl BridgeoutPrecompile {
    pub fn new(fixed_withdrawal_wei: U256) -> Self {
        Self {
            fixed_withdrawal_wei,
        }
    }
}

impl<DB: Database> ContextStatefulPrecompile<DB> for BridgeoutPrecompile {
    fn call(
        &self,
        dest_pk_bytes: &Bytes,
        _gas_limit: u64,
        evmctx: &mut revm::InnerEvmContext<DB>,
    ) -> PrecompileResult {
        // Validate the length of the destination public key
        let dest_pk = try_into_pubkey(dest_pk_bytes)
            .map_err(|_| PrecompileError::other("Invalid public key length: expected 32 bytes"))?;

        // Verify that the transaction value matches the required withdrawal amount
        let withdrawal_amount = evmctx.env.tx.value;
        if withdrawal_amount != self.fixed_withdrawal_wei {
            return Err(PrecompileError::other(
                "Invalid withdrawal value: must be exactly 10 BTC in wei",
            )
            .into());
        }

        // Convert wei to satoshis
        let (sats, _) = wei_to_sats(withdrawal_amount);

        // Try converting sats (U256) into u64 amount
        let amount: u64 = sats.try_into().map_err(|_| PrecompileErrors::Fatal {
            msg: "Withdrawal amount exceeds maximum allowed value".into(),
        })?;

        // Log the bridge withdrawal intent
        let evt = WithdrawalIntentEvent { amount, dest_pk };
        let logdata = LogData::from(&evt);

        evmctx.journaled_state.log(Log {
            address: BRIDGEOUT_ADDRESS,
            data: logdata,
        });

        // Burn value sent to bridge by adjusting the account balance of bridge precompile
        let (account, _) = evmctx
            .load_account(BRIDGEOUT_ADDRESS)
            // Error case should never occur
            .map_err(|_| PrecompileErrors::Fatal {
                msg: "Failed to load BRIDGEOUT_ADDRESS account".into(),
            })?;

        // NOTE: account balance will always be greater or equal to value sent in tx
        let new_balance = account.info.balance.saturating_sub(withdrawal_amount);

        account.info.balance = new_balance;

        // TODO: Properly calculate and deduct gas for the bridge out operation
        let gas_cost = 0;

        Ok(PrecompileOutput::new(gas_cost, Bytes::new()))
    }
}
