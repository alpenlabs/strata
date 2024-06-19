use std::collections::{vec_deque::Iter, VecDeque};

use bitcoin::{hashes::Hash, Block, BlockHash};
use tracing::warn;

use crate::rpc::traits::L1Client;

// FIXME: This could possibly be arg or through config
#[cfg(not(test))]
pub const MAX_REORG_DEPTH: u64 = 6;
#[cfg(test)]
pub const MAX_REORG_DEPTH: u64 = 3;

pub async fn detect_reorg(
    latest_seen_blocks: &VecDeque<BlockHash>,
    block_num: u64,
    block: &Block,
    rpc_client: &impl L1Client,
) -> anyhow::Result<Option<u64>> {
    let exp_prev_hash = block.header.prev_blockhash;

    let mut iter = latest_seen_blocks.iter();
    let prev_hash = iter.next();
    if let Some(hash) = prev_hash {
        if exp_prev_hash == *hash {
            Ok(None)
        } else {
            find_fork_point_until(block_num, &mut iter, rpc_client).await
        }
    } else {
        Ok(None)
    }
}

async fn find_fork_point_until<'a>(
    blk_num: u64,
    prev_blockhashes_iter: &'a mut Iter<'a, BlockHash>,
    rpc_client: &impl L1Client,
) -> anyhow::Result<Option<u64>> {
    let fork_range_start = blk_num - MAX_REORG_DEPTH;

    for height in (fork_range_start..=blk_num).rev() {
        let l1_blk_hash = rpc_client.get_block_hash(height).await?;

        if let Some(block_hash) = prev_blockhashes_iter.next() {
            if *block_hash.as_raw_hash().as_byte_array() == l1_blk_hash {
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

    use alpen_test_utils::ArbitraryGenerator;

    use super::*;

    pub struct TestL1Client {
        pub hashes: Vec<[u8; 32]>,
        pub blocks: Vec<Block>,
    }

    impl TestL1Client {
        pub fn new() -> Self {
            let block1 = "0000002001f742c1ab561c1d26f5378fb777877d8acbd9ccd1546c68e934417d3a7e6715c4faa77f060758654ad09fb84f2d55571af5498faf7cf2e9160189ca7c581a8574710666ffff7f200900000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025600ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";
            let block2 = "0000002052b170c6909b56a5d44744e9ad3bd3063cd7b05fc68a1e3638c608b5a766a86866c38927a3b0f91ae3961a385646ee695d9407bba2bbdefe55da4dec49f1d32574710666ffff7f200500000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025700ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";
            let block3 = "000000205f33261389e85e307364eb7e4cd6a1888a309a20e4afda79ab823d56b529cd22fbac241ca600894b5eb662ded5ecbda6b850c55c4acfb8fccf56dd9d25884bd175710666ffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025800ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";
            let block4 = "000000208b90a523dba04694175d3b9072892e5d2beadfe8794684259cd28fce64a6dc56f6979053ffff7eb93b6965b368b804ca8ffaace177d05598a329c6d71be1530775710666ffff7f200200000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff025900ffffffff0200f2052a01000000160014c7f18c51bf871298d91732590ddd54c6367815aa0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";

            let blocks: Vec<Block> = vec![
                deserialize(&hex::decode(block1).unwrap()).unwrap(),
                deserialize(&hex::decode(block2).unwrap()).unwrap(),
                deserialize(&hex::decode(block3).unwrap()).unwrap(),
                deserialize(&hex::decode(block4).unwrap()).unwrap(),
            ];
            let hashes: Vec<_> = blocks
                .iter()
                .map(|b| b.block_hash().as_raw_hash().as_byte_array().clone())
                .collect();
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

    #[tokio::test]
    async fn test_forkpoint() {
        let forkpoint_depth = 2;
        let client = TestL1Client::new();

        let mut seen_blocks = VecDeque::with_capacity(MAX_REORG_DEPTH as usize);

        // Insert blocks to db that match with what rpc provides, but only upto forkpoint depth
        // The rest will not match with what rpc has
        let total_blocks = client.blocks.len();
        for block in client.blocks[..total_blocks - forkpoint_depth].iter() {
            let _ = seen_blocks.push_front(block.block_hash());
        }

        for _ in 0..forkpoint_depth {
            let mf: L1BlockManifest = ArbitraryGenerator::new().generate();
            let _ = seen_blocks
                .push_front(BlockHash::from_slice(&mf.block_hash().0.as_slice()).unwrap());
        }

        // Now, db and client have same blocks until height 2 and different onwards(until 4)
        let fp = find_fork_point_until(4, &mut seen_blocks.iter(), &client)
            .await
            .unwrap();
        assert_eq!(fp, Some(2));
    }
}
