use std::array::TryFromSliceError;

use revm::{ContextStatefulPrecompile, Database};
use revm_primitives::{
    address, Address, Bytes, FixedBytes, Log, LogData, PrecompileError, PrecompileErrors,
    PrecompileOutput, PrecompileResult, U256,
};

use crate::primitives::WithdrawalIntentEvent;

const fn u256_from(val: u128) -> U256 {
    U256::from_limbs([(val & ((1 << 64) - 1)) as u64, (val >> 64) as u64, 0, 0])
}

/// The address for the Bridgeout precompile contract.
pub const BRIDGEOUT_ADDRESS: Address = address!("000000000000000000000000000000000b121d9e");

/// Number of wei per rollup BTC (1e18).
const WEI_PER_BTC: u128 = 1_000_000_000_000_000_000u128;
/// Number of wei per satoshi (1e10).
const WEI_PER_SAT: U256 = u256_from(10_000_000_000u128);
/// The fixed withdrawal amount in wei (10 BTC equivalent).
const FIXED_WITHDRAWAL_WEI: U256 = u256_from(10 * WEI_PER_BTC);

/// Converts wei to satoshis.
/// Returns a tuple of (satoshis, remainder_in_wei).
fn wei_to_sats(wei: U256) -> (U256, U256) {
    wei.div_rem(WEI_PER_SAT)
}

/// Ensure that input is exactly 32 bytes
fn try_into_pubkey(maybe_pubkey: &Bytes) -> Result<FixedBytes<32>, TryFromSliceError> {
    maybe_pubkey.as_ref().try_into()
}

/// Custom precompile to burn rollup native token and add bridge out intent of equal amount.
/// Bridge out intent is created during block payload generation.
/// This precompile validates transaction and burns the bridge out amount.
pub struct BridgeoutPrecompile;

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
        if withdrawal_amount != FIXED_WITHDRAWAL_WEI {
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

        account.info.balance = account
            .info
            .balance
            .checked_sub(withdrawal_amount)
            // Error case should never occur as `value` has been transfered to this address by evm
            // before running this precompile
            .ok_or_else(|| PrecompileErrors::Fatal {
                msg: "Insufficient balance in BRIDGEOUT_ADDRESS account".into(),
            })?;

        // TODO: Properly calculate and deduct gas for the bridge out operation
        let gas_cost = 0; // Placeholder

        Ok(PrecompileOutput::new(gas_cost, Bytes::new()))
    }
}
