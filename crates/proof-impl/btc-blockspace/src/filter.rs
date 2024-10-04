//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use alpen_express_primitives::{block_credential::CredRule, params::RollupParams};
use alpen_express_state::{
    batch::BatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};
use bitcoin::Block;
use strata_tx_parser::filter::{derive_tx_filter_rules, filter_relevant_txs};

pub fn extract_relevant_info(
    block: &Block,
    rollup_params: &RollupParams,
) -> (Vec<DepositInfo>, Option<BatchCheckpoint>) {
    let filters = derive_tx_filter_rules(rollup_params);

    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    let relevant_txs = filter_relevant_txs(block, &filters);

    for tx in relevant_txs {
        match tx.proto_op() {
            ProtocolOperation::Deposit(deposit_info) => {
                deposits.push(deposit_info.clone());
            }
            ProtocolOperation::RollupInscription(signed_batch) => {
                if let CredRule::SchnorrKey(pub_key) = rollup_params.cred_rule {
                    assert!(signed_batch.verify_sig(pub_key));
                }
                let batch: BatchCheckpoint = signed_batch.clone().into();
                // Note: This assumes we will have one proper update
                prev_checkpoint = prev_checkpoint.or(Some(batch));
            }
            _ => {}
        }
    }

    (deposits, prev_checkpoint)
}
