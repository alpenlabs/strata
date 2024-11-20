use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::bail;
use bitcoin::{hashes::Hash, BlockHash};
use strata_db::traits::Database;
use strata_primitives::buf::Buf32;
use strata_state::l1::{
    get_btc_params, get_difficulty_adjustment_height, BtcParams, HeaderVerificationState,
    L1BlockId, TimestampStore,
};
use strata_status::StatusTx;
use strata_tx_parser::{
    filter::filter_protocol_op_tx_refs,
    filter_types::TxFilterConfig,
    messages::{BlockData, L1Event},
};
use tokio::sync::mpsc;
use tracing::*;

use crate::{
    reader::{config::ReaderConfig, state::ReaderState},
    rpc::traits::Reader,
    status::{apply_status_updates, L1StatusUpdate},
};

// FIXME: remove this when there's actually a chainstate manager
type ChainstateManager = ();

struct ReaderContext<R: Reader> {
    /// Bitcoin reader client
    client: Arc<R>,
    /// L1Event sender
    event_tx: mpsc::Sender<L1Event>,
    /// Config
    config: Arc<ReaderConfig>,
    /// Status transmitter
    status_tx: Arc<StatusTx>,
    /// Chainstate manager
    chainstate_mgr: ChainstateManager, // TODO: actual type
}

// TODO: remove this
pub async fn bitcoin_data_reader_task<D: Database + 'static>(
    client: Arc<impl Reader>,
    event_tx: mpsc::Sender<L1Event>,
    target_next_block: u64,
    config: Arc<ReaderConfig>,
    status_tx: Arc<StatusTx>,
    // TODO: replace this with actual chainstate manager
    _chainstate_mgr: Arc<D::ChainstateDB>,
) -> anyhow::Result<()> {
    let ctx = ReaderContext {
        client,
        event_tx,
        config,
        status_tx,
        chainstate_mgr: (), // TODO: actual type
    };
    do_reader_task(ctx, target_next_block).await
}

async fn do_reader_task<R: Reader>(
    ctx: ReaderContext<R>,
    target_next_block: u64,
) -> anyhow::Result<()> {
    info!(%target_next_block, "started L1 reader task!");

    let poll_dur = Duration::from_millis(ctx.config.client_poll_dur_ms as u64);

    let mut state = init_reader_state(
        target_next_block,
        ctx.config.max_reorg_depth as usize * 2,
        ctx.client.as_ref(),
    )
    .await?;
    let best_blkid = state.best_block();
    info!(%best_blkid, "initialized L1 reader state");

    loop {
        let mut status_updates: Vec<L1StatusUpdate> = Vec::new();
        let cur_best_height = state.best_block_idx();
        let poll_span = debug_span!("l1poll", %cur_best_height);

        if let Err(err) = poll_for_new_blocks(&ctx, &mut state, &mut status_updates)
            .instrument(poll_span)
            .await
        {
            warn!(%cur_best_height, err = %err, "failed to poll Bitcoin client");
            status_updates.push(L1StatusUpdate::RpcError(err.to_string()));

            if let Some(err) = err.downcast_ref::<reqwest::Error>() {
                // recoverable errors
                if err.is_connect() {
                    status_updates.push(L1StatusUpdate::RpcConnected(false));
                }
                // unrecoverable errors
                if err.is_builder() {
                    panic!("btcio: couldn't build the L1 client");
                }
            }
        }

        tokio::time::sleep(poll_dur).await;

        status_updates.push(L1StatusUpdate::LastUpdate(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        ));

        apply_status_updates(&status_updates, ctx.status_tx.clone()).await;
    }
}

/// Inits the reader state by trying to backfill blocks up to a target height.
async fn init_reader_state(
    target_next_block: u64,
    lookback: usize,
    client: &impl Reader,
) -> anyhow::Result<ReaderState> {
    // Init the reader state using the blockid we were given, fill in a few blocks back.
    debug!(%target_next_block, "initializing reader state");
    let mut init_queue = VecDeque::new();

    // Do some math to figure out where our start and end are.
    // TODO something screwed up with bookkeeping here
    let chain_info = client.get_blockchain_info().await?;
    let start_height = i64::max(target_next_block as i64 - lookback as i64, 0) as u64;
    let end_height = u64::min(target_next_block - 1, chain_info.blocks);
    debug!(%start_height, %end_height, "queried L1 client, have init range");

    // Loop through the range we've determined to be okay and pull the blocks we want to look back
    // through in.
    let mut real_cur_height = start_height;
    for height in start_height..=end_height {
        let blkid = client.get_block_hash(height).await?;
        debug!(%height, %blkid, "loaded recent L1 block");
        init_queue.push_back(blkid);
        real_cur_height = height;
    }

    let state = ReaderState::new(real_cur_height + 1, lookback, init_queue);
    Ok(state)
}

/// Polls the chain to see if there's new blocks to look at, possibly reorging
/// if there's a mixup and we have to go back.
async fn poll_for_new_blocks<R: Reader>(
    ctx: &ReaderContext<R>,
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
) -> anyhow::Result<()> {
    let chain_info = ctx.client.get_blockchain_info().await?;
    status_updates.push(L1StatusUpdate::RpcConnected(true));
    let client_height = chain_info.blocks;
    let fresh_best_block = chain_info.best_block_hash.parse::<BlockHash>()?;

    status_updates.push(L1StatusUpdate::CurHeight(client_height));
    status_updates.push(L1StatusUpdate::CurTip(fresh_best_block.to_string()));

    if fresh_best_block == *state.best_block() {
        trace!("polled client, nothing to do");
        return Ok(());
    }

    // First, check for a reorg if there is one.
    if let Some((pivot_height, pivot_blkid)) = find_pivot_block(ctx.client.as_ref(), state).await? {
        if pivot_height < state.best_block_idx() {
            info!(%pivot_height, %pivot_blkid, "found apparent reorg");
            state.rollback_to_height(pivot_height);
            let revert_ev = L1Event::RevertTo(pivot_height);
            if ctx.event_tx.send(revert_ev).await.is_err() {
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
    let scan_start_height = state.next_height();
    for fetch_height in scan_start_height..=client_height {
        let l1blkid = match fetch_and_process_block(ctx, fetch_height, state, status_updates).await
        {
            Ok(b) => b,
            Err(e) => {
                warn!(%fetch_height, err = %e, "failed to fetch new block");
                break;
            }
        };
        info!(%fetch_height, %l1blkid, "accepted new block");
    }

    Ok(())
}

/// Finds the highest block index where we do agree with the node.  If we never
/// find one then we're really screwed.
async fn find_pivot_block(
    client: &impl Reader,
    state: &ReaderState,
) -> anyhow::Result<Option<(u64, BlockHash)>> {
    for (height, l1blkid) in state.iter_blocks_back() {
        // If at genesis, we can't reorg any farther.
        if height == 0 {
            return Ok(Some((height, *l1blkid)));
        }

        let queried_l1blkid = client.get_block_hash(height).await?;
        trace!(%height, %l1blkid, %queried_l1blkid, "comparing blocks to find pivot");
        if queried_l1blkid == *l1blkid {
            return Ok(Some((height, *l1blkid)));
        }
    }

    Ok(None)
}

async fn fetch_and_process_block<R: Reader>(
    ctx: &ReaderContext<R>,
    height: u64,
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
) -> anyhow::Result<BlockHash> {
    let block = ctx.client.get_block_at(height).await?;
    let txs = block.txdata.len();

    let params = ctx.config.params.clone();
    let filter_config = TxFilterConfig::derive_from(params.rollup())?;
    let filtered_txs = filter_protocol_op_tx_refs(&block, filter_config);
    let block_data = BlockData::new(height, block, filtered_txs);
    let l1blkid = block_data.block().block_hash();
    trace!(%height, %l1blkid, %txs, "fetched block from client");

    status_updates.push(L1StatusUpdate::CurHeight(height));
    status_updates.push(L1StatusUpdate::CurTip(l1blkid.to_string()));

    let threshold = params.rollup().l1_reorg_safe_depth;
    let genesis_ht = params.rollup().genesis_l1_height;
    let genesis_threshold = genesis_ht + threshold as u64;

    trace!(%genesis_ht, %threshold, %genesis_threshold, "should genesis?");

    if height == genesis_threshold {
        info!(%height, %genesis_ht, "time for genesis");
        let l1_verification_state =
            get_verification_state(ctx.client.as_ref(), genesis_ht + 1, &get_btc_params()).await?;
        if let Err(e) = ctx
            .event_tx
            .send(L1Event::GenesisVerificationState(
                height,
                l1_verification_state,
            ))
            .await
        {
            error!("failed to submit L1 block event, did the persistence task crash?");
            return Err(e.into());
        }
    }

    if let Err(e) = ctx.event_tx.send(L1Event::BlockData(block_data)).await {
        error!("failed to submit L1 block event, did the persistence task crash?");
        return Err(e.into());
    }

    // Insert to new block, incrementing cur_height.
    let _deep = state.accept_new_block(l1blkid);

    Ok(l1blkid)
}

/// Gets the [`HeaderVerificationState`] for the particular block
pub async fn get_verification_state(
    client: &impl Reader,
    height: u64,
    params: &BtcParams,
) -> anyhow::Result<HeaderVerificationState> {
    // Get the difficulty adjustment block just before `block_height`
    let h1 = get_difficulty_adjustment_height(0, height as u32, params);
    let b1 = client.get_block_at(h1 as u64).await?;

    // Consider the block before `block_height` to be the last verified block
    let vh = height - 1; // verified_height
    let vb = client.get_block_at(vh).await?; // verified_block

    // Fetch the previous timestamps of block from `vh`
    // This fetches timestamps of `vh`, `vh-1`, `vh-2`, ...
    const N: usize = 11;
    let mut timestamps: [u32; 11] = [0u32; 11];
    for i in (0..N).rev() {
        if vh > i as u64 {
            let h = client.get_block_at(vh - i as u64).await?;
            timestamps[i] = h.header.time;
        } else {
            timestamps[i] = 0;
        }
    }
    let last_11_blocks_timestamps = TimestampStore::new(timestamps);

    let l1_blkid: L1BlockId =
        Buf32::from(vb.header.block_hash().as_raw_hash().to_byte_array()).into();
    Ok(HeaderVerificationState {
        last_verified_block_num: vh as u32,
        last_verified_block_hash: l1_blkid,
        next_block_target: vb.header.target().to_compact_lossy().to_consensus(),
        interval_start_timestamp: b1.header.time,
        total_accumulated_pow: 0u128,
        last_11_blocks_timestamps,
    })
}
