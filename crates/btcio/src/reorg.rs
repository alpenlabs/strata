use std::sync::Arc;

use alpen_vertex_db::traits::L1DataProvider;
use alpen_vertex_primitives::buf::Buf32;
use tracing::warn;

use crate::{reader::BlockData, rpc::traits::L1Client};

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
            find_fork_point_until(blockdata.block_num(), db, rpc_client).await
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
#[cfg(not(test))]
const MAX_REORG_DEPTH: u64 = 6;
#[cfg(test)]
const MAX_REORG_DEPTH: u64 = 3;

async fn find_fork_point_until<D>(
    blk_num: u64,
    db: &Arc<D>,
    rpc_client: &impl L1Client,
) -> anyhow::Result<Option<u64>>
where
    D: L1DataProvider,
{
    let fork_range_start = blk_num - MAX_REORG_DEPTH;

    for height in (fork_range_start..=blk_num).rev() {
        let l1_blk_hash = rpc_client.get_block_hash(height).await?;
        if let Some(block_mf) = db.get_block_manifest(height)? {
            let hash = block_mf.block_hash();
            if hash == l1_blk_hash.into() {
                return Ok(Some(height));
            }
        } else {
            warn!(%height, "L1Db could not get manifest for height");
            break;
        }
    }

    Err(anyhow::anyhow!(
        "Could not find fork point event until MAX_REORG_DEPTH limit"
    ))
}

#[cfg(test)]
mod tests {
    use alpen_vertex_primitives::l1::L1BlockManifest;
    use async_trait::async_trait;
    use bitcoin::{consensus::deserialize, hashes::Hash, Block};

    use alpen_test_utils::{get_rocksdb_tmp_instance, ArbitraryGenerator};
    use alpen_vertex_db::{traits::L1DataStore, L1Db};

    use crate::handlers::block_to_manifest;

    use super::*;

    pub struct TestL1Client {
        pub hashes: Vec<[u8; 32]>,
        pub blocks: Vec<Block>,
    }

    impl TestL1Client {
        pub fn new() -> Self {
            let block1_hash = "0000002001f742c1ab561c1d26f5378fb777877d8acbd9ccd1546c68e934417d3a7e6715c4faa77f060758654ad09fb84f2d55571af5498faf7cf2e9160189ca7c581a8574710666ffff7f200900000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025600ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";
            let block2_hash = "0000002052b170c6909b56a5d44744e9ad3bd3063cd7b05fc68a1e3638c608b5a766a86866c38927a3b0f91ae3961a385646ee695d9407bba2bbdefe55da4dec49f1d32574710666ffff7f200500000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025700ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";
            let block3_hash = "000000205f33261389e85e307364eb7e4cd6a1888a309a20e4afda79ab823d56b529cd22fbac241ca600894b5eb662ded5ecbda6b850c55c4acfb8fccf56dd9d25884bd175710666ffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025800ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";
            let block4_hash = "000000208b90a523dba04694175d3b9072892e5d2beadfe8794684259cd28fce64a6dc56f6979053ffff7eb93b6965b368b804ca8ffaace177d05598a329c6d71be1530775710666ffff7f200200000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025900ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";

            let blocks: Vec<Block> = vec![
                deserialize(&hex::decode(block1_hash).unwrap()).unwrap(),
                deserialize(&hex::decode(block2_hash).unwrap()).unwrap(),
                deserialize(&hex::decode(block3_hash).unwrap()).unwrap(),
                deserialize(&hex::decode(block4_hash).unwrap()).unwrap(),
            ];
            let hashes: Vec<_> = blocks
                .iter()
                .map(|b| b.block_hash().as_raw_hash().as_byte_array().clone())
                .collect();
            let bufs: Vec<Buf32> = hashes.iter().map(|&x| x.into()).collect();
            println!("HASHES: {:?}\nBLOCKS: {:?}", bufs, bufs);
            Self { hashes, blocks }
        }
    }

    #[async_trait]
    impl L1Client for TestL1Client {
        async fn get_block_at(&self, height: u64) -> anyhow::Result<Block> {
            Ok(self.blocks[height as usize - 1].clone())
        }

        async fn get_block_hash(&self, height: u64) -> anyhow::Result<[u8; 32]> {
            Ok(self.hashes[height as usize - 1])
        }
    }

    fn setup_db() -> L1Db {
        let db = get_rocksdb_tmp_instance().unwrap();
        L1Db::new(db)
    }

    #[tokio::test]
    async fn test_forkpoint() {
        let forkpoint_depth = 2;
        let db = Arc::new(setup_db());
        let client = TestL1Client::new();

        // Insert blocks to db that match with what rpc provides, but only upto forkpoint depth
        // The rest will not match with what rpc has
        let total_blocks = client.blocks.len();
        let mut height = 1;
        for block in client.blocks[..total_blocks - forkpoint_depth].iter() {
            let mf = block_to_manifest(block.clone());
            let _ = db.put_block_data(height, mf, vec![]).unwrap();
            height += 1;
        }

        for _ in 0..forkpoint_depth {
            let mf: L1BlockManifest = ArbitraryGenerator::new().generate();
            let _ = db.put_block_data(height, mf, vec![]).unwrap();
            height += 1;
        }

        // Now, db and client have same blocks until height 2 and different onwards(until 4)
        let fp = find_fork_point_until(4, &db, &client).await.unwrap();
        assert_eq!(fp, Some(2));
    }
}
