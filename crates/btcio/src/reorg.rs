use std::sync::Arc;

use alpen_vertex_db::traits::L1DataProvider;
use alpen_vertex_primitives::buf::Buf32;

use crate::{
    reader::BlockData,
    rpc::{traits::L1Client, BitcoinClient},
};

pub async fn detect_reorg<D>(
    db: &Arc<D>,
    blockdata: &BlockData,
    rpc_client: &impl L1Client,
) -> anyhow::Result<Option<u64>>
where
    D: L1DataProvider,
{
    let exp_prev_hash: Buf32 = blockdata.block().header.prev_blockhash.into();
    let prev_hash = get_prev_block_hash(blockdata.block_num(), db)?;
    if let Some(hash) = prev_hash {
        if exp_prev_hash == hash {
            Ok(None)
        } else {
            find_fork_point_before(db, blockdata.block_num(), rpc_client).await
        }
    } else {
        Ok(None)
    }
}

fn get_prev_block_hash<D>(curr_blk_num: u64, db: &Arc<D>) -> anyhow::Result<Option<Buf32>>
where
    D: L1DataProvider,
{
    let block_mf = db.get_block_manifest(curr_blk_num - 1)?;
    Ok(block_mf.map(|x| x.block_hash()))
}

// FIXME: This could possibly be arg or through config
const MAX_REORG_DEPTH: u64 = 6;

async fn find_fork_point_before<D>(
    db: &Arc<D>,
    blk_num: u64,
    rpc_client: &impl L1Client,
) -> anyhow::Result<Option<u64>>
where
    D: L1DataProvider,
{
    let fork_range_start = blk_num - MAX_REORG_DEPTH;

    for height in (fork_range_start..blk_num).rev() {
        let l1_blk_hash = rpc_client.get_block_hash(height).await?;
        if let Some(block_mf) = db.get_block_manifest(height)? {
            let hash = block_mf.block_hash();
            if hash == l1_blk_hash.into() {
                return Ok(Some(height));
            }
        } else {
            break;
        }
    }

    Err(anyhow::anyhow!(
        "Could not find fork point event until MAX_REORG_DEPTH limit"
    ))
}
