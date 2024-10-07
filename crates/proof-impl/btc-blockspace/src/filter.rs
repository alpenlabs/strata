//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use bitcoin::Block;
use strata_primitives::block_credential::CredRule;
use strata_state::{
    batch::BatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};
use strata_tx_parser::filter::{filter_relevant_txs, TxFilterRule};

pub fn extract_relevant_info(
    block: &Block,
    tx_filters: &[TxFilterRule],
    cred_rule: &CredRule,
) -> (Vec<DepositInfo>, Option<BatchCheckpoint>) {
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    let relevant_txs = filter_relevant_txs(block, tx_filters);

    for tx in relevant_txs {
        match tx.proto_op() {
            ProtocolOperation::Deposit(deposit_info) => {
                deposits.push(deposit_info.clone());
            }
            ProtocolOperation::RollupInscription(signed_batch) => {
                if let CredRule::SchnorrKey(pub_key) = cred_rule {
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
