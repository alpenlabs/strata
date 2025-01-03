//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use bitcoin::Block;
use strata_primitives::{block_credential::CredRule, params::RollupParams};
use strata_state::{
    batch::BatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};
use strata_tx_parser::filter::{filter_protocol_op_tx_refs, TxFilterConfig};

pub fn extract_relevant_info(
    block: &Block,
    rollup_params: &RollupParams,
) -> (Vec<DepositInfo>, Option<BatchCheckpoint>) {
    let filter_config =
        TxFilterConfig::derive_from(rollup_params).expect("derive tx-filter config");

    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    let relevant_txs = filter_protocol_op_tx_refs(block, &filter_config);

    for tx in relevant_txs {
        match tx.proto_op() {
            ProtocolOperation::Deposit(deposit_info) => {
                deposits.push(deposit_info.clone());
            }
            ProtocolOperation::Checkpoint(signed_batch) => {
                if let CredRule::SchnorrKey(pub_key) = rollup_params.cred_rule {
                    assert!(signed_batch.verify_sig(&pub_key));
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
