use std::sync::Arc;

use alpen_vertex_db::traits::L1DataProvider;
use bitcoin::Block;

pub fn detect_reorg<D>(db: &Arc<D>, block: &Block) -> Option<u64>
where
    D: L1DataProvider,
{
    todo!()
}
