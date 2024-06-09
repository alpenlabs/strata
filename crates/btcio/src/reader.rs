use std::{sync::Arc, time::Duration};

use alpen_vertex_db::traits::L1DataProvider;
use bitcoin::Block;
use tokio::sync::mpsc;

use crate::rpc::BitcoinClient;

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
    sender: mpsc::Sender<BlockData>,
    l1_start_block_height: u64,
) -> anyhow::Result<()>
where
    D: L1DataProvider,
{
    loop {
        // Get next block num to fetch, if it's 0 start with l1_start_block_height
        // Since reorg is handled by handler function, it's best to get the current best height from
        // l1db.
        let curr_block_num = l1db.get_chain_tip()?;
        let next_block_num = if curr_block_num == 0 {
            l1_start_block_height
        } else {
            curr_block_num + 1
        };

        let next_block = client.get_block_at(next_block_num).await?;

        let filtered_block_indices = filter_relevant_txns(&next_block);

        let block_data = BlockData {
            block_num: next_block_num,
            block: next_block,
            relevant_txn_indices: filtered_block_indices,
        };
        let _ = sender.send(block_data).await?;

        let _ = tokio::time::sleep(Duration::new(1, 0));
    }
}
