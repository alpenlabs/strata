//! Core logic of the Bitcoin Blockspace proof that will be proven

use bitcoin::{consensus::serialize, Block};
use strata_l1tx::filter::TxFilterConfig;
use strata_primitives::params::RollupParams;
use strata_state::l1::L1TxProof;

use crate::{block::check_integrity, filter::extract_relevant_info, logic::BlockScanResult};

#[inline]
pub fn process_blockscan(
    block: &Block,
    inclusion_proof: &Option<L1TxProof>,
    rollup_params: &RollupParams,
    filter_config: &TxFilterConfig,
) -> BlockScanResult {
    assert!(check_integrity(block, inclusion_proof));

    let (deposits, prev_checkpoint) = extract_relevant_info(block, rollup_params, filter_config);

    BlockScanResult {
        header_raw: serialize(&block.header),
        deposits,
        prev_checkpoint,
    }
}
