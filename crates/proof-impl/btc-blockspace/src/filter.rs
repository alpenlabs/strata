//! This includes all the filtering logic to filter out and extract
//! deposits, forced inclusion transactions as well as state updates

use bitcoin::Block;
use strata_l1tx::filter::{filter_protocol_op_tx_refs, visitor::OpsVisitor, TxFilterConfig};
use strata_primitives::{block_credential::CredRule, params::RollupParams};
use strata_state::{
    batch::BatchCheckpoint,
    tx::{DepositInfo, ProtocolOperation},
};

/// Ops visitor for Prover.
pub struct ProverOpsVisitor;

impl OpsVisitor for ProverOpsVisitor {}

pub fn extract_relevant_info(
    block: &Block,
    rollup_params: &RollupParams,
    filter_config: &TxFilterConfig,
) -> (Vec<DepositInfo>, Option<BatchCheckpoint>) {
    let mut deposits = Vec::new();
    let mut prev_checkpoint = None;

    // Just pass a no-op to the filter function as prover does not have to do anything with the raw
    // data like storing in db.
    let txrefs = filter_protocol_op_tx_refs(block, rollup_params, filter_config, &ProverOpsVisitor);

    for op in txrefs.into_iter().flat_map(|t| t.proto_ops().to_vec()) {
        match op {
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
