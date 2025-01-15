use alloy_sol_types::SolEvent;
use reth_primitives::Receipt;
use revm_primitives::U256;
use strata_primitives::bitcoin_bosd::Descriptor;
use strata_reth_primitives::{WithdrawalIntent, WithdrawalIntentEvent};

use crate::constants::BRIDGEOUT_ADDRESS;

pub const fn u256_from(val: u128) -> U256 {
    U256::from_limbs([(val & ((1 << 64) - 1)) as u64, (val >> 64) as u64, 0, 0])
}

/// Number of wei per rollup BTC (1e18).
pub const WEI_PER_BTC: u128 = 1_000_000_000_000_000_000u128;

/// Number of wei per satoshi (1e10).
const WEI_PER_SAT: U256 = u256_from(10_000_000_000u128);

/// Converts wei to satoshis.
/// Returns a tuple of (satoshis, remainder_in_wei).
pub fn wei_to_sats(wei: U256) -> (U256, U256) {
    wei.div_rem(WEI_PER_SAT)
}

/// Collects withdrawal intents from bridge-out events in the receipts.
/// Returns a vector of optional [`WithdrawalIntent`]s.
///
/// # Note
///
/// A [`Descriptor`], if invalid does not create an [`WithdrawalIntent`].
pub fn collect_withdrawal_intents(
    receipts: impl Iterator<Item = Option<Receipt>>,
) -> impl Iterator<Item = Option<WithdrawalIntent>> {
    receipts
        .flatten()
        .flat_map(|receipt| receipt.logs)
        .filter(|log| log.address == BRIDGEOUT_ADDRESS)
        .filter_map(|log| {
            WithdrawalIntentEvent::decode_log(&log, true)
                .map(|evt| {
                    let descriptor = Descriptor::from_bytes(&evt.destination);
                    match descriptor {
                        Ok(valid_descriptor) => Some(WithdrawalIntent {
                            amt: evt.amount,
                            destination: valid_descriptor,
                        }),
                        Err(_) => None,
                    }
                })
                .ok()
        })
}
