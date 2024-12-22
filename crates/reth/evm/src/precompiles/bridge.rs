use revm::{ContextStatefulPrecompile, Database};
use revm_primitives::{
    Bytes, FixedBytes, Log, LogData, PrecompileError, PrecompileErrors, PrecompileOutput,
    PrecompileResult, U256,
};
use strata_reth_primitives::WithdrawalIntentEvent;

pub use crate::constants::BRIDGEOUT_ADDRESS;
use crate::utils::wei_to_sats;

const CALLDATA_LENGTH: usize = 32 + 4;

/// Custom precompile to burn rollup native token and add bridge out intent of equal amount.
/// Bridge out intent is created during block payload generation.
/// This precompile validates the transaction and burns the bridge out amount.
pub struct BridgeoutPrecompile {
    fixed_withdrawal_wei: U256,
}

impl BridgeoutPrecompile {
    pub fn new(fixed_withdrawal_wei: U256) -> Self {
        Self {
            fixed_withdrawal_wei,
        }
    }

    /// Validates the calldata length and extracts the txid and vout from the provided calldata.
    fn parse_calldata(calldata: &Bytes) -> Result<(FixedBytes<32>, u32), PrecompileError> {
        if calldata.len() != CALLDATA_LENGTH {
            return Err(PrecompileError::other(format!(
                "Invalid calldata length: expected {} bytes, got {}",
                CALLDATA_LENGTH,
                calldata.len()
            )));
        }

        let calldata: [u8; CALLDATA_LENGTH] = calldata
            .as_ref()
            .try_into()
            .expect("Failed to convert calldata into fixed array");

        let txid: FixedBytes<32> = calldata[0..32]
            .try_into()
            .expect("FixedBytes size mismatch");
        let vout = u32::from_be_bytes(calldata[32..36].try_into().expect("u32 conversion failed"));

        Ok((txid, vout))
    }

    /// Converts a U256 amount from wei to satoshis and checks if it fits into u64.
    fn calculate_amount_in_sats(withdrawal_amount: U256) -> Result<u64, PrecompileErrors> {
        let (sats, _) = wei_to_sats(withdrawal_amount);
        sats.try_into().map_err(|_| PrecompileErrors::Fatal {
            msg: "Withdrawal amount exceeds maximum allowed value".into(),
        })
    }
}

impl<DB: Database> ContextStatefulPrecompile<DB> for BridgeoutPrecompile {
    fn call(
        &self,
        calldata: &Bytes,
        _gas_limit: u64,
        evmctx: &mut revm::InnerEvmContext<DB>,
    ) -> PrecompileResult {
        // Parse and validate calldata
        let (txid, vout) = Self::parse_calldata(calldata)?;

        // Verify that the transaction value matches the required withdrawal amount
        let withdrawal_amount = evmctx.env.tx.value;
        if withdrawal_amount != self.fixed_withdrawal_wei {
            return Err(PrecompileError::other(
                "Invalid withdrawal value: must be exactly the specified withdrawal amount",
            )
            .into());
        }

        // Calculate withdrawal amount in satoshis
        let amount = Self::calculate_amount_in_sats(withdrawal_amount)?;

        // Log the bridge withdrawal intent
        let evt = WithdrawalIntentEvent { amount, txid, vout };
        let logdata = LogData::from(&evt);
        evmctx.journaled_state.log(Log {
            address: BRIDGEOUT_ADDRESS,
            data: logdata,
        });

        // Adjust the account balance of the bridge precompile
        let (account, _) =
            evmctx
                .load_account(BRIDGEOUT_ADDRESS)
                .map_err(|_| PrecompileErrors::Fatal {
                    msg: "Failed to load BRIDGEOUT_ADDRESS account".into(),
                })?;

        account.info.balance = account.info.balance.saturating_sub(withdrawal_amount);

        // TODO: Properly calculate and deduct gas for the bridge out operation
        let gas_cost = 0;

        Ok(PrecompileOutput::new(gas_cost, Bytes::new()))
    }
}
