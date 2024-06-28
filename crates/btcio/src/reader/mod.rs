pub mod config;
pub mod handler;
pub mod reorg;

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use bitcoin::{Block, BlockHash};
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

/// State we use in various parts of the reader.
struct ReaderState {
    /// The highest block in the chain, at `.back()` of queue.
    cur_height: u64,

    /// The `.back()` of this should have the same height as cur_height.
    seen_blocks: VecDeque<BlockHash>,

    max_depth: usize,
}

impl ReaderState {
    fn new(cur_height: u64, max_depth: usize, seen_blocks: VecDeque<BlockHash>) -> Self {
        Self {
            cur_height,
            max_depth,
            seen_blocks,
        }
    }

    /// Accepts a new block and purges a buried one.
    fn accept_new_block(&mut self, blkhash: BlockHash) -> Option<BlockHash> {
        let ret = if self.seen_blocks.len() > self.max_depth {
            Some(self.seen_blocks.pop_front().unwrap())
        } else {
            None
        };

        self.seen_blocks.push_back(blkhash);
        self.cur_height += 1;
        ret
    }

    /// Gets the blockhash of the given height, if we have it.
    pub fn get_height_blkid(&self, height: u64) -> Option<&BlockHash> {
        if height > self.cur_height {
            return None;
        }

        if height < self.deepest_block() {
            return None;
        }

        let off = self.seen_blocks.len() as u64 - height;
        Some(&self.seen_blocks[off as usize])
    }

    fn deepest_block(&self) -> u64 {
        self.cur_height - self.seen_blocks.len() as u64
    }

    fn revert_tip(&mut self) -> Option<BlockHash> {
        if !self.seen_blocks.is_empty() {
            let back = self.seen_blocks.pop_back().unwrap();
            self.cur_height -= 1;
            Some(back)
        } else {
            None
        }
    }

    fn rollback_to_height(&mut self, new_height: u64) -> Vec<BlockHash> {
        if new_height > self.cur_height {
            panic!("reader: new height greater than cur height");
        }

        let rollback_cnt = self.cur_height - new_height;
        if rollback_cnt > self.seen_blocks.len() as u64 {
            panic!("reader: tried to rollback past deepest block");
        }

        let mut buf = Vec::new();
        for _ in 0..rollback_cnt {
            let blkhash = self.revert_tip().expect("reader: rollback tip");
            buf.push(blkhash);
        }

        buf
    }
}

pub async fn bitcoin_data_reader_task(
    client: impl L1Client,
    event_tx: mpsc::Sender<L1Event>,
    cur_block_height: u64,
    config: Arc<ReaderConfig>,
) -> anyhow::Result<()> {
    let poll_dur = Duration::from_millis(config.client_poll_dur_ms as u64);

    let mut state = ReaderState::new(cur_block_height, 12, VecDeque::new());

    // FIXME This function will return when reorg happens when there are not
    // enough elements in the vec deque, probably during startup.
    loop {
        match poll_rpc_for_new_blocks(&client, &event_tx, &config, &mut state).await {
            Ok(_) => {}
            Err(e) => {
                warn!(err = %e, "failed to poll Bitcoin client");
            }
        }

        tokio::time::sleep(poll_dur).await;
    }
}

async fn poll_rpc_for_new_blocks(
    client: &impl L1Client,
    event_tx: &mpsc::Sender<L1Event>,
    config: &ReaderConfig,
    state: &mut ReaderState,
) -> anyhow::Result<()> {
    let block = client.get_block_at(state.cur_height).await?;

    // TODO Make a query to getbestblockhash to short-circuit any logic and
    // avoid doing any more work than we have to.

    // Check to see if we just witnessed the client doing a reorg.
    let reorg_res = reorg::detect_reorg(
        &state.seen_blocks,
        state.cur_height,
        &block,
        client,
        &config,
    )
    .await?;

    if let Some(reorg_blk_num) = reorg_res {
        let cur_blk_num = state.cur_height;
        // TODO verify that cur_blk_num is the pivot block

        warn!(%cur_blk_num, %reorg_blk_num, "observed apparent L1 reorg");
        let _reverted = state.rollback_to_height(cur_blk_num);

        if let Err(e) = event_tx.send(L1Event::RevertTo(reorg_blk_num)).await {
            error!("failed to submit L1 reorg event, did the persistence task crash?");
            return Err(e.into());
        }

        return Ok(());
    }

    let filtered_txs = filter_interesting_txs(&block);
    let block_data = BlockData {
        block_num: state.cur_height,
        block,
        interesting_tx_idxs: filtered_txs,
    };

    let block_hash = block_data.block().block_hash();

    if let Err(e) = event_tx.send(L1Event::BlockData(block_data)).await {
        error!("failed to submit L1 block event, did the persistence task crash?");
        return Err(e.into());
    }

    // Insert to new block, incrementing cur_height.
    let _deep = state.accept_new_block(block_hash);

    Ok(())
}
