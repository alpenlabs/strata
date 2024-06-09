use std::time::Duration;

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

pub async fn bitcoin_data_reader(
    client: BitcoinClient,
    sender: mpsc::Sender<BlockData>,
    start_block_number: u64,
) -> anyhow::Result<()> {
    let mut next_block_num = start_block_number;
    loop {
        let next_block = client.get_block_at(next_block_num).await?;

        let filtered_block_indices = filter_relevant_txns(&next_block);

        let block_data = BlockData {
            block_num: next_block_num,
            block: next_block,
            relevant_txn_indices: filtered_block_indices,
        };
        let _ = sender.send(block_data).await?;

        let _ = tokio::time::sleep(Duration::new(1, 0));

        next_block_num += 1;
    }
}
