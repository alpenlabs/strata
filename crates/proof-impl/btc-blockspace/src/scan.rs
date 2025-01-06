//! This module contains the logic to process a blockscan and extract relevant information from it.

use bitcoin::{consensus::serialize, Block};
use strata_primitives::params::RollupParams;

use crate::{block::check_merkle_root, filter::extract_relevant_info, logic::BlockScanResult};

/// Scans a Bitcoin block to extract rollup-relevant data, including checkpoints and deposits.
pub fn process_blockscan(block: &Block, rollup_params: &RollupParams) -> BlockScanResult {
    assert!(check_merkle_root(block));

    // TODO: Assert witness commitment check
    // https://alpenlabs.atlassian.net/browse/STR-758
    // assert!(check_witness_commitment(block));

    let (deposits, prev_checkpoint) = extract_relevant_info(block, rollup_params);

    BlockScanResult {
        header_raw: serialize(&block.header),
        deposits,
        prev_checkpoint,
    }
}

#[cfg(test)]
mod tests {
    use strata_test_utils::{bitcoin::get_btc_chain, l2::gen_params};

    use super::process_blockscan;
    #[test]
    fn test_process_blockspace_proof() {
        let params = gen_params();
        let rollup_params = params.rollup();
        let btc_block = get_btc_chain().get_block(40321).clone();
        let _ = process_blockscan(&btc_block, rollup_params);
    }
}
