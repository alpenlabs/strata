//! L1 data operation interface.

use std::sync::Arc;

use strata_db::traits::*;
use strata_primitives::l1::{L1BlockManifest, L1TxRef};
use strata_state::l1::{L1BlockId, L1Tx};

use crate::exec::*;

inst_ops_simple! {
    (<D: L1Database> => L1DataOps) {
        put_block_data(mf: L1BlockManifest, txs: Vec<L1Tx>) => ();
        // put_mmr_checkpoint(blockid: L1BlockId, mmr: CompactMmr) => ();
        extend_canonical_chain(height: u64, blockid: L1BlockId) => ();
        revert_canonical_chain(height: u64) => ();
        prune_to_height(height: u64) => ();
        get_chain_tip() => Option<(u64, L1BlockId)>;
        get_block_manifest(blockid: L1BlockId) => Option<L1BlockManifest>;
        get_canonical_blockid(height: u64) => Option<L1BlockId>;
        get_canonical_blockid_range(start_height: u64, end_height: u64) => Vec<L1BlockId>;
        get_block_txs(blockid: L1BlockId) => Option<Vec<L1TxRef>>;
        get_tx(tx_ref: L1TxRef) => Option<L1Tx>;
        // get_mmr(blockid: L1BlockId) => Option<CompactMmr>;
    }
}
