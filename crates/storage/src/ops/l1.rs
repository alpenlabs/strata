//! L1 data operation interface.

use std::sync::Arc;

use strata_db::traits::*;
use strata_primitives::l1::{L1BlockId, L1BlockManifest, L1Tx, L1TxRef};

use crate::exec::*;

inst_ops_simple! {
    (<D: L1Database> => L1DataOps) {
        put_block_data(idx: u64, mf: L1BlockManifest, txs: Vec<L1Tx>) => ();
        revert_to_height(idx: u64) => ();
        get_chain_tip() => Option<u64>;
        get_block_manifest(idx: u64) => Option<L1BlockManifest>;
        get_blockid_range(start_idx: u64, end_idx: u64) => Vec<L1BlockId>;
        get_block_txs(idx: u64) => Option<Vec<L1TxRef>>;
        get_tx(tx_ref: L1TxRef) => Option<L1Tx>;
    }
}
