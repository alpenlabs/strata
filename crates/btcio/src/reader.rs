use std::{sync::Arc, time::Duration};

use alpen_vertex_db::traits::L1DataProvider;
use bitcoin::Block;
use tokio::sync::mpsc;

use crate::{reorg::detect_reorg, rpc::traits::L1Client};

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    relevant_txn_indices: Vec<u32>,
}

pub enum L1Data {
    /// Data that contains block number, block and relevent transactions
    BlockData(BlockData),

    /// Revert to the provided block height
    RevertTo(u64),
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
    client: impl L1Client,
    sender: mpsc::Sender<L1Data>,
    current_block_height: u64,
) -> anyhow::Result<()>
where
    D: L1DataProvider,
{
    let mut curr_block_num = current_block_height + 1;
    loop {
        let block = client.get_block_at(curr_block_num).await?;

        if let Some(reorg_block_num) = detect_reorg(&l1db, curr_block_num, &block, &client).await? {
            sender.send(L1Data::RevertTo(reorg_block_num)).await?;
            curr_block_num = reorg_block_num + 1;
            continue;
        }

        let filtered_block_indices = filter_relevant_txns(&block);

        let block_data = BlockData {
            block_num: curr_block_num,
            block,
            relevant_txn_indices: filtered_block_indices,
        };
        let _ = sender.send(L1Data::BlockData(block_data)).await?;
        curr_block_num += 1;

        let _ = tokio::time::sleep(Duration::new(1, 0));
    }
}
