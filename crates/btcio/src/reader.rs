use std::{sync::Arc, time::Duration};

use alpen_vertex_db::traits::L1DataProvider;
use bitcoin::Block;
use tokio::sync::mpsc;

use crate::{reorg::detect_reorg, rpc::BitcoinClient};

const HASH_BLOCK: &str = "hashblock";
const RAW_BLOCK: &str = "rawblock";
const RAW_TX: &str = "rawtx";

const SUBSCRIPTION_TOPICS: &[&'static str] = &[HASH_BLOCK, RAW_BLOCK, RAW_TX];

pub enum L1Data {
    /// Bitcoin block data obtained in normal operation when there's no reorg
    BlockData(BlockData),

    /// Reorg happened, revert to the specified block number
    Reorg(u64),
}

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    relevant_txn_indices: Vec<u32>,
}

impl BlockData {
    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn relevant_txn_indices(&self) -> &Vec<u32> {
        &self.relevant_txn_indices
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}

fn filter_relevant_txns(block: &Block) -> Vec<u32> {
    // TODO: actually implement the filter logic. Now it returns everything
    block
        .txdata
        .iter()
        .enumerate()
        .map(|(i, _)| i as u32)
        .collect()
}

pub async fn bitcoin_data_reader<D>(
    l1db: Arc<D>,
    client: BitcoinClient,
    sender: mpsc::Sender<L1Data>,
) -> anyhow::Result<()>
where
    D: L1DataProvider,
{
    loop {
        let last_block_num = l1db.get_chain_tip()?;
        let next_block_num = last_block_num + 1;

        let next_block = client.get_block_at(next_block_num).await?;

        // TODO: possibly handle when reorg depth is too big
        if let Some(reorg_block_num) = detect_reorg(&l1db, &next_block) {
            let _ = sender.send(L1Data::Reorg(reorg_block_num)).await?;
            continue;
        }

        let filtered_block_indices = filter_relevant_txns(&next_block);

        let block_data = BlockData {
            block_num: next_block_num,
            block: next_block,
            relevant_txn_indices: filtered_block_indices,
        };
        let _ = sender.send(L1Data::BlockData(block_data)).await?;

        let _ = tokio::time::sleep(Duration::new(1, 0));
    }
}
