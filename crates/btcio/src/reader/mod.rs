pub mod config;
pub mod handler;
pub mod reorg;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use bitcoin::Block;
use tokio::sync::mpsc;
use tracing::*;

use self::config::ReaderConfig;
use crate::rpc::traits::L1Client;

/// Store the bitcoin block and references to the relevant transactions within the block
#[derive(Clone, Debug)]
pub struct BlockData {
    block_num: u64,
    block: Block,
    interesting_tx_idxs: Vec<u32>,
}

/// L1 events that we observe and want the persistence task to work on.
#[derive(Clone, Debug)]
pub enum L1Event {
    /// Data that contains block number, block and relevent transactions
    BlockData(BlockData),

    /// Revert to the provided block height
    RevertTo(u64),
}

impl BlockData {
    pub fn block(&self) -> &Block {
        &self.block
    }

    pub fn interesting_tx_idxs(&self) -> &[u32] {
        &self.interesting_tx_idxs
    }

    pub fn block_num(&self) -> u64 {
        self.block_num
    }
}

fn filter_interesting_txs(block: &Block) -> Vec<u32> {
    // TODO actually implement the filter logic. Now it returns everything
    // TODO maybe this should be on the persistence side?
    (0..=block.txdata.len()).map(|i| i as u32).collect()
}

pub async fn bitcoin_data_reader_task(
    client: impl L1Client,
    event_tx: mpsc::Sender<L1Event>,
    current_block_height: u64,
    config: Arc<ReaderConfig>,
) -> anyhow::Result<()> {
    let mut cur_blk_num = current_block_height + 1;

    // FIXME If there's an RPC error this whole task will crash.  We should do
    // some backoff but not fail completely if the RPC call fails for
    // recoverable errors (like the Bitcoin client just being restarted).

    // TODO This should probably be initialized with what's present in the l1db
    // upto max_reorg_depth.
    let mut seen_blocks: VecDeque<_> = VecDeque::with_capacity(config.max_reorg_depth as usize);

    // FIXME This function will return when reorg happens when there are not
    // enough elements in the vec deque, probably during startup.
    loop {
        let block = client.get_block_at(cur_blk_num).await?;

        // TODO Make a query to getbestblockhash to short-circuit any logic and
        // avoid doing any more work than we have to.

        // Check to see if we just witnessed the client doing a reorg.
        if let Some(reorg_blk_num) =
            reorg::detect_reorg(&seen_blocks, cur_blk_num, &block, &client, &config).await?
        {
            warn!(%cur_blk_num, %reorg_blk_num, "observed apparent L1 reorg");

            if let Err(e) = event_tx.send(L1Event::RevertTo(reorg_blk_num)).await {
                error!("failed to submit L1 reorg event, did the persistence task crash?");
                break Err(e.into());
            }

            cur_blk_num = reorg_blk_num + 1;
            continue;
        }

        let filtered_txs = filter_interesting_txs(&block);

        let block_data = BlockData {
            block_num: cur_blk_num,
            block,
            interesting_tx_idxs: filtered_txs,
        };

        let block_hash = block_data.block().block_hash();

        if let Err(e) = event_tx.send(L1Event::BlockData(block_data)).await {
            error!("failed to submit L1 block event, did the persistence task crash?");
            break Err(e.into());
        }

        // Insert to seen_blocks and increment curr_block_num.
        //
        // This uses the queue backwards, but it means that the iterator works
        // how we'd want it to, so it's fine.
        if seen_blocks.len() == seen_blocks.capacity() {
            seen_blocks.pop_back();
        }
        seen_blocks.push_front(block_hash);
        cur_blk_num += 1;

        tokio::time::sleep(Duration::from_millis(config.client_poll_dur_ms as u64)).await;
    }
}
