use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::bail;
use bitcoin::BlockHash;
use strata_db::traits::Database;
use strata_primitives::params::Params;
use strata_status::StatusTx;
use tokio::sync::mpsc;
use tracing::*;

use crate::{
    reader::{
        config::ReaderConfig,
        filter::{filter_relevant_txs, RelevantTxType},
        messages::{BlockData, L1Event},
        state::ReaderState,
    },
    rpc::traits::Reader,
    status::{apply_status_updates, L1StatusUpdate},
};

pub async fn bitcoin_data_reader_task<D: Database + 'static>(
    client: Arc<impl Reader>,
    event_tx: mpsc::Sender<L1Event>,
    target_next_block: u64,
    config: Arc<ReaderConfig>,
    status_rx: Arc<StatusTx>,
    chstate_prov: Arc<D::ChsProv>,
    params: Arc<Params>,
) {
    if let Err(e) = do_reader_task::<D>(
        client.as_ref(),
        &event_tx,
        target_next_block,
        config,
        status_rx.clone(),
        chstate_prov,
        params,
    )
    .await
    {
        error!(err = %e, "reader task exited");
    }
}

async fn do_reader_task<D: Database + 'static>(
    client: &impl Reader,
    event_tx: &mpsc::Sender<L1Event>,
    target_next_block: u64,
    config: Arc<ReaderConfig>,
    status_rx: Arc<StatusTx>,
    chstate_prov: Arc<D::ChsProv>,
    params: Arc<Params>,
) -> anyhow::Result<()> {
    info!(%target_next_block, "started L1 reader task!");

    let poll_dur = Duration::from_millis(config.client_poll_dur_ms as u64);

    let mut state = init_reader_state(
        target_next_block,
        config.max_reorg_depth as usize * 2,
        client,
    )
    .await?;
    let best_blkid = state.best_block();
    info!(%best_blkid, "initialized L1 reader state");

    loop {
        let mut status_updates: Vec<L1StatusUpdate> = Vec::new();
        let cur_best_height = state.best_block_idx();
        let poll_span = debug_span!("l1poll", %cur_best_height);

        // Maybe this should be called outside loop?
        let relevant_tx_types =
            derive_relevant_tx_types::<D>(chstate_prov.clone(), params.as_ref())?;

        if let Err(err) = poll_for_new_blocks(
            client,
            event_tx,
            &relevant_tx_types,
            &mut state,
            &mut status_updates,
        )
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

        apply_status_updates(&status_updates, status_rx.clone()).await;
    }
}

fn derive_relevant_tx_types<D: Database + 'static>(
    _chstate_prov: Arc<D::ChsProv>,
    params: &Params,
) -> anyhow::Result<Vec<RelevantTxType>> {
    // TODO: Figure out how to do it from chainstate provider
    // For now we'll just go with filtering Inscription transactions
    Ok(vec![RelevantTxType::RollupInscription(
        params.rollup().rollup_name.clone(),
    )])
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
async fn poll_for_new_blocks(
    client: &impl Reader,
    event_tx: &mpsc::Sender<L1Event>,
    relevant_tx_types: &[RelevantTxType],
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
) -> anyhow::Result<()> {
    let chain_info = client.get_blockchain_info().await?;
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
    if let Some((pivot_height, pivot_blkid)) = find_pivot_block(client, state).await? {
        if pivot_height < state.best_block_idx() {
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
    let scan_start_height = state.next_height();
    for fetch_height in scan_start_height..=client_height {
        let l1blkid = match fetch_and_process_block(
            fetch_height,
            client,
            event_tx,
            state,
            status_updates,
            relevant_tx_types,
        )
        .await
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

async fn fetch_and_process_block(
    height: u64,
    client: &impl Reader,
    event_tx: &mpsc::Sender<L1Event>,
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
    relevant_tx_types: &[RelevantTxType],
) -> anyhow::Result<BlockHash> {
    let block = client.get_block_at(height).await?;
    let txs = block.txdata.len();

    let filtered_txs = filter_relevant_txs(&block, relevant_tx_types);
    let block_data = BlockData::new(height, block, filtered_txs);
    let l1blkid = block_data.block().block_hash();
    trace!(%l1blkid, %height, %txs, "fetched block from client");

    status_updates.push(L1StatusUpdate::CurHeight(height));
    status_updates.push(L1StatusUpdate::CurTip(l1blkid.to_string()));
    if let Err(e) = event_tx.send(L1Event::BlockData(block_data)).await {
        error!("failed to submit L1 block event, did the persistence task crash?");
        return Err(e.into());
    }

    // Insert to new block, incrementing cur_height.
    let _deep = state.accept_new_block(l1blkid);

    Ok(l1blkid)
}
