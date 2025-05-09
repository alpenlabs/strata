use alloy_sol_types::SolEvent;
use reth_primitives::{Receipt, TransactionSigned};
use revm_primitives::U256;
use strata_primitives::{bitcoin_bosd::Descriptor, buf::Buf32};
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

/// Tuple of executed transaction and receipt
pub type TxReceiptPair<'a> = (&'a TransactionSigned, &'a Receipt);

/// Collects withdrawal intents from bridge-out events by matching
/// executed transactions (for txid) and receipts.
/// Returns a vector of [`WithdrawalIntent`]s.
///
/// # Note
///
/// A [`Descriptor`], if invalid does not create an [`WithdrawalIntent`].
pub fn collect_withdrawal_intents<'a, I>(
    tx_receipt_pairs: I,
) -> impl Iterator<Item = WithdrawalIntent> + 'a
where
    I: Iterator<Item = TxReceiptPair<'a>> + 'a,
{
    tx_receipt_pairs.flat_map(|(tx, receipt)| {
        receipt.logs.iter().filter_map(move |log| {
            if log.address != BRIDGEOUT_ADDRESS {
                return None;
            }

            let txid = Buf32(tx.hash().as_slice().try_into().expect("32 bytes"));
            WithdrawalIntentEvent::decode_log(log).ok().and_then(|evt| {
                Descriptor::from_bytes(&evt.destination)
                    .ok()
                    .map(|destination| WithdrawalIntent {
                        amt: evt.amount,
                        destination,
                        withdrawal_txid: txid,
                    })
            })
        })
    })
}
