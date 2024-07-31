use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alpen_express_primitives::l1::L1Status;
use anyhow::bail;
use bitcoin::{Block, BlockHash};
use tokio::sync::{mpsc, RwLock};
use tracing::*;

use super::config::ReaderConfig;
use super::messages::{BlockData, L1Event};
use crate::rpc::traits::L1Client;
use crate::status::{apply_status_updates, StatusUpdate};

fn filter_interesting_txs(block: &Block) -> Vec<u32> {
    // TODO actually implement the filter logic. Now it returns everything
    // TODO maybe this should be on the persistence side?
    (0..=block.txdata.len()).map(|i| i as u32).collect()
}

/// State we use in various parts of the reader.
#[derive(Debug)]
struct ReaderState {
    /// The highest block in the chain, at `.back()` of queue.
    cur_height: u64,

    /// The `.back()` of this should have the same height as cur_height.
    recent_blocks: VecDeque<BlockHash>,

    max_depth: usize,
}

impl ReaderState {
    fn new(cur_height: u64, max_depth: usize, recent_blocks: VecDeque<BlockHash>) -> Self {
        assert!(!recent_blocks.is_empty());
        Self {
            cur_height,
            max_depth,
            recent_blocks,
        }
    }

    fn best_block(&self) -> &BlockHash {
        self.recent_blocks.back().unwrap()
    }

    /// Accepts a new block and purges a buried one.
    fn accept_new_block(&mut self, blkhash: BlockHash) -> Option<BlockHash> {
        let ret = if self.recent_blocks.len() > self.max_depth {
            Some(self.recent_blocks.pop_front().unwrap())
        } else {
            None
        };

        self.recent_blocks.push_back(blkhash);
        self.cur_height += 1;
        ret
    }

    #[allow(unused)]
    /// Gets the blockhash of the given height, if we have it.
    pub fn get_height_blkid(&self, height: u64) -> Option<&BlockHash> {
        if height > self.cur_height {
            return None;
        }

        if height < self.deepest_block() {
            return None;
        }

        let back_off = self.cur_height - height;
        let idx = self.recent_blocks.len() as u64 - back_off - 1;
        Some(&self.recent_blocks[idx as usize])
    }
    #[allow(unused)]
    fn deepest_block(&self) -> u64 {
        self.cur_height - self.recent_blocks.len() as u64 - 1
    }

    fn revert_tip(&mut self) -> Option<BlockHash> {
        if !self.recent_blocks.is_empty() {
            let back = self.recent_blocks.pop_back().unwrap();
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
        if rollback_cnt >= self.recent_blocks.len() as u64 {
            panic!("reader: tried to rollback past deepest block");
        }

        let mut buf = Vec::new();
        for _ in 0..rollback_cnt {
            let blkhash = self.revert_tip().expect("reader: rollback tip");
            buf.push(blkhash);
        }

        // More sanity checks.
        assert!(!self.recent_blocks.is_empty());
        assert_eq!(self.cur_height, new_height);

        buf
    }

    /// Iterates over the blocks back from the tip, giving both the height and
    /// the blockhash to compare against the chain.
    fn iter_blocks_back(&self) -> impl Iterator<Item = (u64, &BlockHash)> {
        self.recent_blocks
            .iter()
            .rev()
            .enumerate()
            .map(|(i, b)| (self.cur_height - i as u64, b))
    }
}

pub async fn bitcoin_data_reader_task(
    client: impl L1Client,
    event_tx: mpsc::Sender<L1Event>,
    cur_block_height: u64,
    config: Arc<ReaderConfig>,
    l1_status: Arc<RwLock<L1Status>>,
) {
    let mut status_updates = Vec::new();
    if let Err(e) = do_reader_task(
        &client,
        &event_tx,
        cur_block_height,
        config,
        &mut status_updates,
        l1_status.clone(),
    )
    .await
    {
        error!(err = %e, "reader task exited");
    }
}

async fn do_reader_task(
    client: &impl L1Client,
    event_tx: &mpsc::Sender<L1Event>,
    cur_block_height: u64,
    config: Arc<ReaderConfig>,
    status_updates: &mut Vec<StatusUpdate>,
    l1_status: Arc<RwLock<L1Status>>,
) -> anyhow::Result<()> {
    info!(%cur_block_height, "started L1 reader task!");

    let poll_dur = Duration::from_millis(config.client_poll_dur_ms as u64);

    let mut state = init_reader_state(
        cur_block_height,
        config.max_reorg_depth as usize * 2,
        client,
    )
    .await?;
    let best_blkid = state.best_block();
    info!(%best_blkid, "initialized L1 reader state");

    // FIXME This function will return when reorg happens when there are not
    // enough elements in the vec deque, probably during startup.
    loop {
        let cur_height = state.cur_height;
        let poll_span = debug_span!("l1poll", %cur_height);

        if let Err(err) = poll_for_new_blocks(client, event_tx, &config, &mut state, status_updates)
            .instrument(poll_span)
            .await
        {
            warn!(%cur_height, err = %err, "failed to poll Bitcoin client");
            status_updates.push(StatusUpdate::RpcError(err.to_string()));

            if let Some(err) = err.downcast_ref::<reqwest::Error>() {
                // recoverable errors
                if err.is_connect() {
                    status_updates.push(StatusUpdate::RpcConnected(false));
                }
                // unrecoverable errors
                if err.is_builder() {
                    panic!("btcio: couldn't build the L1 client");
                }
            }
        }

        tokio::time::sleep(poll_dur).await;

        status_updates.push(StatusUpdate::LastUpdate(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        ));

        apply_status_updates(status_updates, l1_status.clone()).await;
    }
}

/// Inits the reader state by trying to backfill blocks up to a target height.
async fn init_reader_state(
    target_block: u64,
    lookback: usize,
    client: &impl L1Client,
) -> anyhow::Result<ReaderState> {
    // Init the reader state using the blockid we were given, fill in a few blocks back.
    debug!(%target_block, "initializing reader state");
    let mut init_queue = VecDeque::new();

    // Do some math to figure out where our start and end are.
    let chain_info = client.get_blockchain_info().await?;
    let start_height = i64::max(target_block as i64 - lookback as i64, 0) as u64;
    let end_height = u64::min(target_block, chain_info.blocks);
    debug!(%start_height, %end_height, "queried L1 client, have init range");

    // Loop through the range we've determined to be okay and pull the blocks
    // in.
    let mut real_cur_height = start_height;
    for height in start_height..=end_height {
        let blkid = client.get_block_hash(height).await?;
        debug!(%height, %blkid, "loaded recent L1 block");
        init_queue.push_back(blkid);
        real_cur_height = height;
    }

    let state = ReaderState::new(real_cur_height, lookback, init_queue);
    Ok(state)
}

/// Polls the chain to see if there's new blocks to look at, possibly reorging
/// if there's a mixup and we have to go back.
async fn poll_for_new_blocks(
    client: &impl L1Client,
    event_tx: &mpsc::Sender<L1Event>,
    _config: &ReaderConfig,
    state: &mut ReaderState,
    status_updates: &mut Vec<StatusUpdate>,
) -> anyhow::Result<()> {
    let chain_info = client.get_blockchain_info().await?;
    status_updates.push(StatusUpdate::RpcConnected(true));
    let client_height = chain_info.blocks;
    let fresh_best_block = chain_info.bestblockhash();

    status_updates.push(StatusUpdate::CurHeight(client_height));
    status_updates.push(StatusUpdate::CurTip(fresh_best_block.to_string()));

    if fresh_best_block == *state.best_block() {
        trace!("polled client, nothing to do");
        return Ok(());
    }

    // First, check for a reorg if there is one.
    if let Some((pivot_height, pivot_blkid)) = find_pivot_block(client, state).await? {
        if pivot_height < state.cur_height {
            info!(%pivot_height, %pivot_blkid, "found apparent reorg");
            state.rollback_to_height(pivot_height);
            let revert_ev = L1Event::RevertTo(pivot_height);
            if event_tx.send(revert_ev).await.is_err() {
                warn!("unable to submit L1 reorg event, did persistence task exit?");
            }
        }
    } else {
        // TODO make this case a bit more structured
        error!("unable to find common block with client chain, something is seriously wrong here!");
        bail!("things are broken");
    }

    debug!(%client_height, "have new blocks");

    // Now process each block we missed.
    let scan_start_height = state.cur_height + 1;
    for fetch_height in scan_start_height..=client_height {
        let blkid =
            match fetch_and_process_block(fetch_height, client, event_tx, state, status_updates)
                .await
            {
                Ok(b) => b,
                Err(e) => {
                    warn!(%fetch_height, err = %e, "failed to fetch new block");
                    break;
                }
            };
        info!(%fetch_height, %blkid, "accepted new block");
    }

    Ok(())
}

/// Finds the highest block index where we do agree with the node.  If we never
/// find one then we're really screwed.
async fn find_pivot_block(
    client: &impl L1Client,
    state: &ReaderState,
) -> anyhow::Result<Option<(u64, BlockHash)>> {
    for (height, blkid) in state.iter_blocks_back() {
        // If at genesis, we can't reorg any farther.
        if height == 0 {
            return Ok(Some((height, *blkid)));
        }

        let queried_blkid = client.get_block_hash(height).await?;
        trace!(%height, %blkid, %queried_blkid, "comparing blocks to find pivot");
        if queried_blkid == *blkid {
            return Ok(Some((height, *blkid)));
        }
    }

    Ok(None)
}

async fn fetch_and_process_block(
    height: u64,
    client: &impl L1Client,
    event_tx: &mpsc::Sender<L1Event>,
    state: &mut ReaderState,
    status_updates: &mut Vec<StatusUpdate>,
) -> anyhow::Result<BlockHash> {
    let block = client.get_block_at(height).await?;

    let filtered_txs = filter_interesting_txs(&block);
    let block_data = BlockData::new(height, block, filtered_txs);
    let blkid = block_data.block().block_hash();

    status_updates.push(StatusUpdate::CurHeight(height));
    status_updates.push(StatusUpdate::CurTip(blkid.to_string()));
    if let Err(e) = event_tx.send(L1Event::BlockData(block_data)).await {
        error!("failed to submit L1 block event, did the persistence task crash?");
        return Err(e.into());
    }

    // Insert to new block, incrementing cur_height.
    let _deep = state.accept_new_block(blkid);

    Ok(blkid)
}
