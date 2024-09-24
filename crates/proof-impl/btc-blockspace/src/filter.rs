//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use alpen_express_state::{
    batch::BatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};
use bitcoin::Block;
use strata_tx_parser::filter::{filter_relevant_txs, TxFilterRule};

pub fn extract_relevant_info(
    block: &Block,
    filters: &[TxFilterRule],
) -> (Vec<DepositInfo>, Option<BatchCheckpoint>) {
    let mut deposits = Vec::new();
    let mut state_update = None;

    let relevant_txs = filter_relevant_txs(block, filters);

    for tx in relevant_txs {
        match tx.proto_op() {
            ProtocolOperation::Deposit(deposit_info) => {
                deposits.push(deposit_info.clone());
            }
            ProtocolOperation::RollupInscription(signed_batch) => {
                // TODO: Verify the signature
                // TODO: This assumes we will have one proper update, but there can be many
                let batch: BatchCheckpoint = signed_batch.clone().into();
                state_update = state_update.or(Some(batch));
            }
            _ => {}
        }
    }

    (deposits, state_update)
}
