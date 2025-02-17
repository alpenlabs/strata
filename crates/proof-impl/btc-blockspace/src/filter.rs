//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use bitcoin::Block;
use strata_l1tx::filter::{indexer::index_block, TxFilterConfig};
use strata_primitives::{
    batch::{verify_signed_checkpoint_sig, Checkpoint},
    block_credential::CredRule,
    l1::{DepositInfo, ProtocolOperation},
    params::RollupParams,
};

use crate::tx_indexer::ProverTxVisitorImpl;

// FIXME: needs better name
pub fn extract_relevant_info(
    block: &Block,
    rollup_params: &RollupParams,
    filter_config: &TxFilterConfig,
) -> (Vec<DepositInfo>, Option<Checkpoint>) {
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    let tx_entries = index_block(block, ProverTxVisitorImpl::new, filter_config);

    for op in tx_entries.into_iter().flat_map(|t| t.into_contents()) {
        match op {
            ProtocolOperation::Deposit(deposit_info) => {
                deposits.push(deposit_info.clone());
            }
            ProtocolOperation::Checkpoint(signed_ckpt) => {
                // Verify the signature.
                assert!(verify_signed_checkpoint_sig(&signed_ckpt, rollup_params));

                // Note: This assumes we will have one proper update
                // FIXME: ^what if we have improper updates or more than one proper update?
                let batch: Checkpoint = signed_ckpt.checkpoint().clone();
                prev_checkpoint = prev_checkpoint.or(Some(batch));
            }
            _ => {}
        }
    }

    (deposits, prev_checkpoint)
}
