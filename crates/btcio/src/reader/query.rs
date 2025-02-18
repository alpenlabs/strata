use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::bail;
use bitcoin::{params::Params as BtcParams, Block, BlockHash};
use secp256k1::XOnlyPublicKey;
use strata_config::btcio::ReaderConfig;
use strata_l1tx::{
    filter::{indexer::index_block, TxFilterConfig},
    messages::{BlockData, L1Event, RelevantTxEntry},
};
use strata_primitives::{block_credential::CredRule, l1::L1BlockCommitment, params::Params};
use strata_state::{
    l1::{
        get_difficulty_adjustment_height, EpochTimestamps, HeaderVerificationState, L1BlockId,
        TimestampStore,
    },
    sync_event::EventSubmitter,
};
use strata_status::StatusChannel;
use strata_storage::L1BlockManager;
use tracing::*;

use crate::{
    reader::{handler::handle_bitcoin_event, state::ReaderState, tx_indexer::ReaderTxVisitorImpl},
    rpc::traits::ReaderRpc,
    status::{apply_status_updates, L1StatusUpdate},
};

/// Context that encapsulates common items needed for L1 reader.
pub(crate) struct ReaderContext<R: ReaderRpc> {
    /// Bitcoin reader client
    pub client: Arc<R>,
    /// L1db manager
    pub l1_manager: Arc<L1BlockManager>,
    /// Config
    pub config: Arc<ReaderConfig>,
    /// Params
    pub params: Arc<Params>,
    /// Status transmitter
    pub status_channel: StatusChannel,
    /// Sequencer Pubkey
    pub seq_pubkey: Option<XOnlyPublicKey>,
}

/// The main task that initializes the reader state and starts reading from bitcoin.
pub async fn bitcoin_data_reader_task<E: EventSubmitter>(
    client: Arc<impl ReaderRpc>,
    l1_manager: Arc<L1BlockManager>,
    config: Arc<ReaderConfig>,
    params: Arc<Params>,
    status_channel: StatusChannel,
    event_submitter: Arc<E>,
) -> anyhow::Result<()> {
    let target_next_block =
        calculate_target_next_block(l1_manager.as_ref(), params.rollup().horizon_l1_height)?;

    let seq_pubkey = match params.rollup.cred_rule {
        CredRule::Unchecked => None,
        CredRule::SchnorrKey(buf32) => Some(
            XOnlyPublicKey::try_from(buf32)
                .expect("the sequencer pubkey must be valid in the params"),
        ),
    };

    let ctx = ReaderContext {
        client,
        l1_manager,
        config,
        params,
        status_channel,
        seq_pubkey,
    };
    do_reader_task(ctx, target_next_block, event_submitter.as_ref()).await
}

/// Calculates target next block to start polling l1 from.
fn calculate_target_next_block(
    l1_manager: &L1BlockManager,
    horz_height: u64,
) -> anyhow::Result<u64> {
    // TODO switch to checking the L1 tip in the consensus/client state
    let target_next_block = l1_manager
        .get_chain_tip()?
        .map(|i| i + 1)
        .unwrap_or(horz_height);
    assert!(target_next_block >= horz_height);
    Ok(target_next_block)
}

/// Inner function that actually does the reading task.
async fn do_reader_task<R: ReaderRpc, E: EventSubmitter>(
    ctx: ReaderContext<R>,
    target_next_block: u64,
    event_submitter: &E,
) -> anyhow::Result<()> {
    info!(%target_next_block, "started L1 reader task!");

    let poll_dur = Duration::from_millis(ctx.config.client_poll_dur_ms as u64);
    let mut state = init_reader_state(&ctx, target_next_block).await?;
    let best_blkid = state.best_block();
    info!(%best_blkid, "initialized L1 reader state");

    loop {
        let mut status_updates: Vec<L1StatusUpdate> = Vec::new();

        // See if epoch/filter rules have changed
        if let Some(l1ev) = check_epoch_change(&ctx, &mut state)? {
            handle_bitcoin_event(l1ev, &ctx, event_submitter).await?;
        };

        match poll_for_new_blocks(&ctx, &mut state, &mut status_updates).await {
            Err(err) => {
                handle_poll_error(&err, &mut status_updates);
            }
            Ok(events) => {
                // handle events
                for ev in events {
                    handle_bitcoin_event(ev, &ctx, event_submitter).await?;
                }
            }
        };

        tokio::time::sleep(poll_dur).await;

        status_updates.push(L1StatusUpdate::LastUpdate(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        ));

        apply_status_updates(&status_updates, &ctx.status_channel).await;
    }
}

/// Handles errors encountered during polling.
fn handle_poll_error(err: &anyhow::Error, status_updates: &mut Vec<L1StatusUpdate>) {
    warn!(err = %err, "failed to poll Bitcoin client");
    status_updates.push(L1StatusUpdate::RpcError(err.to_string()));

    if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
        if reqwest_err.is_connect() {
            status_updates.push(L1StatusUpdate::RpcConnected(false));
        }
        if reqwest_err.is_builder() {
            panic!("btcio: couldn't build the L1 client");
        }
    }
}

/// Checks for epoch changes and any L1 reverts necessary. Internally updates reader's state.
fn check_epoch_change<R: ReaderRpc>(
    ctx: &ReaderContext<R>,
    state: &mut ReaderState,
) -> anyhow::Result<Option<L1Event>> {
    let new_epoch = ctx.status_channel.epoch().unwrap_or(0);
    // TODO: check if new_epoch < current epoch. should panic if so?
    let curr_epoch = state.epoch();

    if curr_epoch != new_epoch {
        state.set_epoch(new_epoch);
    }

    // TODO: pass in chainstate to `derive_from`
    let new_config = TxFilterConfig::derive_from(ctx.params.rollup())?;
    let curr_filter_config = state.filter_config().clone();

    if new_config != curr_filter_config {
        state.set_filter_config(new_config.clone());

        let last_checkpt_height = ctx
            .status_channel
            .get_last_checkpoint()
            .expect("got epoch change without checkpoint finalized")
            .height;

        // Now, we need to revert to the point before the last checkpoint height.
        state.rollback_to_height(last_checkpt_height);

        // Create revert event
        Ok(Some(L1Event::RevertTo(last_checkpt_height)))
    } else {
        Ok(None)
    }
}

/// Inits the reader state by trying to backfill blocks up to a target height.
async fn init_reader_state<R: ReaderRpc>(
    ctx: &ReaderContext<R>,
    target_next_block: u64,
) -> anyhow::Result<ReaderState> {
    // Init the reader state using the blockid we were given, fill in a few blocks back.
    debug!(%target_next_block, "initializing reader state");
    let mut init_queue = VecDeque::new();

    let lookback = ctx.params.rollup().l1_reorg_safe_depth as usize * 2;
    let client = ctx.client.as_ref();
    let hor_height = ctx.params.rollup().horizon_l1_height;
    let pre_hor = hor_height.saturating_sub(1);
    let target = target_next_block as i64;

    // Do some math to figure out where our start and end are.
    let chain_info = client.get_blockchain_info().await?;
    let start_height = (target - lookback as i64)
        .max(pre_hor as i64)
        .min(chain_info.blocks as i64) as u64;
    let end_height = chain_info
        .blocks
        .min(pre_hor.max(target_next_block.saturating_sub(1)));
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

    let params = ctx.params.clone();
    let filter_config = TxFilterConfig::derive_from(params.rollup())?;
    let epoch = ctx.status_channel.epoch().unwrap_or(0);
    let state = ReaderState::new(
        real_cur_height + 1,
        lookback,
        init_queue,
        filter_config,
        epoch,
    );
    Ok(state)
}

/// Polls the chain to see if there's new blocks to look at, possibly reorging
/// if there's a mixup and we have to go back. Returns events corresponding to block and
/// transactions.
async fn poll_for_new_blocks<R: ReaderRpc>(
    ctx: &ReaderContext<R>,
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
) -> anyhow::Result<Vec<L1Event>> {
    let chain_info = ctx.client.get_blockchain_info().await?;
    status_updates.push(L1StatusUpdate::RpcConnected(true));
    let client_height = chain_info.blocks;
    let fresh_best_block = chain_info.best_block_hash.parse::<BlockHash>()?;

    if fresh_best_block == *state.best_block() {
        trace!("polled client, nothing to do");
        return Ok(vec![]);
    }

    let mut events = Vec::new();

    // First, check for a reorg if there is one.
    if let Some((pivot_height, pivot_blkid)) = find_pivot_block(ctx.client.as_ref(), state).await? {
        if pivot_height < state.best_block_idx() {
            info!(%pivot_height, %pivot_blkid, "found apparent reorg");
            state.rollback_to_height(pivot_height);
            let revert_ev = L1Event::RevertTo(pivot_height);
            // Return with the revert event immediately
            return Ok(vec![revert_ev]);
        }
    } else {
        // TODO make this case a bit more structured
        error!("unable to find common block with client chain, something is seriously wrong here!");
        bail!("things are broken with l1 reader");
    }

    debug!(%client_height, "have new blocks");

    // Now process each block we missed.
    let scan_start_height = state.next_height();
    for fetch_height in scan_start_height..=client_height {
        match fetch_and_process_block(ctx, fetch_height, state, status_updates).await {
            Ok((blkid, evs)) => {
                events.extend_from_slice(&evs);
                info!(%fetch_height, %blkid, "accepted new block");
            }
            Err(e) => {
                warn!(%fetch_height, err = %e, "failed to fetch new block");
                break;
            }
        };
    }

    Ok(events)
}

/// Finds the highest block index where we do agree with the node.  If we never
/// find one then we're really screwed.
async fn find_pivot_block(
    client: &impl ReaderRpc,
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

/// Fetches a block at given height, extracts relevant transactions and emits an `L1Event`.
async fn fetch_and_process_block<R: ReaderRpc>(
    ctx: &ReaderContext<R>,
    height: u64,
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
) -> anyhow::Result<(BlockHash, Vec<L1Event>)> {
    let block = ctx.client.get_block_at(height).await?;
    let (evs, l1blkid) = process_block(ctx, state, status_updates, height, block).await?;

    // Insert to new block, incrementing cur_height.
    let _deep = state.accept_new_block(l1blkid);

    Ok((l1blkid, evs))
}

/// Processes a bitcoin Block to return corresponding `L1Event` and `BlockHash`.
async fn process_block<R: ReaderRpc>(
    ctx: &ReaderContext<R>,
    state: &mut ReaderState,
    status_updates: &mut Vec<L1StatusUpdate>,
    height: u64,
    block: Block,
) -> anyhow::Result<(Vec<L1Event>, BlockHash)> {
    let txs = block.txdata.len();
    let params = ctx.params.clone();

    // Index all the stuff in the block.
    let entries: Vec<RelevantTxEntry> =
        index_block(&block, ReaderTxVisitorImpl::new, state.filter_config());

    // TODO: do stuffs with dep_reqs and da_entries

    let block_data = BlockData::new(height, block, entries);

    let l1blkid = block_data.block().block_hash();
    trace!(%height, %l1blkid, %txs, "fetched block from client");

    status_updates.push(L1StatusUpdate::CurHeight(height));
    status_updates.push(L1StatusUpdate::CurTip(l1blkid.to_string()));

    let threshold = params.rollup().l1_reorg_safe_depth;
    let genesis_ht = params.rollup().genesis_l1_height;
    let genesis_threshold = genesis_ht + threshold as u64;

    trace!(%genesis_ht, %threshold, %genesis_threshold, "should genesis?");

    let block_ev = L1Event::BlockData(block_data, state.epoch());
    let mut l1_events = vec![block_ev];

    if height == genesis_threshold {
        info!(%height, %genesis_ht, "time for genesis");
        let l1_verification_state = get_verification_state(
            ctx.client.as_ref(),
            genesis_ht + 1,
            &BtcParams::new(params.rollup().network),
            params.rollup().l1_reorg_safe_depth,
        )
        .await?;
        let ev = L1Event::GenesisVerificationState(height, l1_verification_state);
        l1_events.push(ev);
    }

    Ok((l1_events, l1blkid))
}

/// Retrieves the timestamps of a specified number of blocks from a given height in descending
/// order.
///
/// This fetches timestamps of `height`, `height-1`, `height-2`, …, down to `height - count + 1` but
/// returns them in ascending order so that the oldest timestamp is first and the newest is last.
///
/// If the provided `height` is less than 1, returns a vector containing a single timestamp of 0.
async fn fetch_block_timestamps_ascending(
    client: &impl ReaderRpc,
    height: u64,
    count: usize,
) -> anyhow::Result<Vec<u32>> {
    let mut timestamps = Vec::with_capacity(count);

    for i in 0..count {
        let current_height = height.saturating_sub(i as u64);
        // If we've gone past block 1, push 0 as a placeholder.
        if current_height < 1 {
            timestamps.push(0);
        } else {
            let header = client.get_block_header_at(current_height).await?;
            timestamps.push(header.time);
        }
    }

    timestamps.reverse();
    Ok(timestamps)
}

/// Gets the [`HeaderVerificationState`] for the particular block
pub async fn get_verification_state(
    client: &impl ReaderRpc,
    height: u64,
    params: &BtcParams,
    l1_reorg_safe_depth: u32,
) -> anyhow::Result<HeaderVerificationState> {
    // Get the difficulty adjustment block just before `block_height`
    let h1 = get_difficulty_adjustment_height(0, height, params);
    let b1 = client.get_block_header_at(h1).await?;

    // Get the difficulty adjustment block just before `h0`
    let h0 = if h1 > params.difficulty_adjustment_interval() {
        h1 - params.difficulty_adjustment_interval()
    } else {
        h1
    };
    let b0 = client.get_block_header_at(h0).await.unwrap_or(b1);

    // Consider the block before `block_height` to be the last verified block
    let vh = height - 1; // verified_height
    let vb = client.get_block_at(vh).await?; // verified_block

    // Bitcoin consensus rule requires last 11 timestamps. We set the count to accomodate for the
    // reorg depth
    const N: usize = 11;
    let count = N + l1_reorg_safe_depth as usize;
    let timestamps = fetch_block_timestamps_ascending(client, vh, count).await?;

    // Calculate the 'head' index for the ring buffer based on the current block height.
    // The 'head' represents the position in the buffer where the next timestamp will be inserted.
    let head = height as usize % count;
    let block_timestamp_history = TimestampStore::new_with_head(&timestamps, head);

    let l1_blkid: L1BlockId = vb.header.block_hash().into();

    let header_vs = HeaderVerificationState {
        last_verified_block: L1BlockCommitment::new(vh, l1_blkid),
        next_block_target: vb.header.target().to_compact_lossy().to_consensus(),
        epoch_timestamps: EpochTimestamps {
            current: b1.time,
            previous: b0.time,
        },
        total_accumulated_pow: 0u128,
        block_timestamp_history,
    };
    trace!(%height, ?header_vs, "HeaderVerificationState");

    Ok(header_vs)
}

#[cfg(test)]
mod test {
    use bitcoin::{hashes::Hash, params::REGTEST};
    use strata_primitives::{buf::Buf32, l1::L1Status};
    use strata_rocksdb::{test_utils::get_rocksdb_tmp_instance, L1Db};
    use strata_state::{
        chain_state::Chainstate,
        client_state::{ClientState, L1Checkpoint},
    };
    use strata_test_utils::{l2::gen_params, ArbitraryGenerator};
    use threadpool::ThreadPool;

    use super::*;
    use crate::test_utils::{
        corepc_node_helpers::{get_bitcoind_and_client, mine_blocks},
        TestBitcoinClient,
    };

    /// Used to populate recent blocks in reader state.
    const N_RECENT_BLOCKS: usize = 10;

    fn get_reader_ctx(chs: Chainstate, cls: ClientState) -> ReaderContext<TestBitcoinClient> {
        let mut gen = ArbitraryGenerator::new();
        let l1status: L1Status = gen.generate();
        let status_channel = StatusChannel::new(cls, l1status, Some(chs));
        let params = Arc::new(gen_params());
        let config = Arc::new(ReaderConfig::default());
        let client = Arc::new(TestBitcoinClient::new(1));

        let (rbdb, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let l1_db = Arc::new(L1Db::new(rbdb.clone(), db_ops));
        let pool = ThreadPool::new(1);
        let l1_manager = Arc::new(L1BlockManager::new(pool, l1_db));
        ReaderContext {
            l1_manager,
            config,
            status_channel,
            params,
            client,
            seq_pubkey: None,
        }
    }

    // Get reader state with provided recent blocks
    fn get_reader_state(
        ctx: &ReaderContext<TestBitcoinClient>,
        n_recent_blocks: usize,
    ) -> ReaderState {
        let filter_config = TxFilterConfig::derive_from(ctx.params.rollup()).unwrap();
        let recent_block_ids: Vec<Buf32> = (0..n_recent_blocks)
            .map(|_| ArbitraryGenerator::new().generate())
            .collect();
        let recent_blocks: VecDeque<BlockHash> = recent_block_ids
            .into_iter()
            .map(|b| BlockHash::from_byte_array(b.into()))
            .collect();
        ReaderState::new(
            n_recent_blocks as u64 + 1, // next height
            n_recent_blocks,
            recent_blocks,
            filter_config,
            ctx.status_channel.epoch().unwrap(),
        )
    }

    /// Check if updating chainstate with new epoch updates reader's state or not when filter config
    /// remain unchanged
    #[tokio::test]
    async fn test_epoch_change() {
        let mut chstate: Chainstate = ArbitraryGenerator::new().generate();
        let clstate: ClientState = ArbitraryGenerator::new().generate();
        let curr_epoch = chstate.cur_epoch();
        println!("curr epoch {:?}", curr_epoch);

        let ctx = get_reader_ctx(chstate.clone(), clstate);
        let mut state = get_reader_state(&ctx, N_RECENT_BLOCKS);

        // Update new chainstate from status channel
        chstate.set_epoch(curr_epoch + 1);
        ctx.status_channel.update_chainstate(chstate);

        let ev = check_epoch_change(&ctx, &mut state).unwrap();

        assert!(
            ev.is_none(),
            "There should be no L1 event if filter config has not changed"
        );

        // The state's epoch should be updated
        assert_eq!(state.epoch(), curr_epoch + 1);
    }

    /// Checks that when new epoch occurs with new tx filter config, reverts the reader state back
    /// to the height of last finalized checkpoint.
    #[tokio::test]
    async fn test_new_filter_rule() {
        let mut chstate: Chainstate = ArbitraryGenerator::new().generate();
        let curr_epoch = chstate.cur_epoch();

        // Create client state with a finalized checkpoint
        let mut clstate: ClientState = ArbitraryGenerator::new().generate();
        let mut checkpt: L1Checkpoint = ArbitraryGenerator::new().generate();

        let chkpt_height = N_RECENT_BLOCKS as u64 - 5; // within recent blocks range, else panics
        checkpt.height = chkpt_height;
        clstate.set_last_finalized_checkpoint(checkpt);

        // Create reader context and state
        let mut ctx = get_reader_ctx(chstate.clone(), clstate.clone());
        let mut state = get_reader_state(&ctx, N_RECENT_BLOCKS);

        // Update status channel with client state
        ctx.status_channel.update_client_state(clstate);

        let old_filter_config = state.filter_config().clone();

        // Simulate tx filter change by updating rollup params. This is because filter config
        // currently depends only on the rollup params and not on the chainstate.
        ctx.params = {
            let mut p = ctx.params.as_ref().clone();
            p.rollup.da_tag = "new-da-tag".to_string();
            p.rollup.checkpoint_tag = "new-ckpt-tag".to_string();
            Arc::new(p)
        };

        // Update new chainstate from status channel
        chstate.set_epoch(curr_epoch + 1);
        ctx.status_channel.update_chainstate(chstate);

        // Check for epoch change, this should not trigger L1 revert because filter config has not
        // changed
        let ev = check_epoch_change(&ctx, &mut state).unwrap();
        assert!(
            matches!(ev, Some(L1Event::RevertTo(_))),
            "Should receive revert event"
        );

        // Check the reader state's next_height
        assert_eq!(
            state.next_height(),
            chkpt_height + 1,
            "Reader's next sheight should be updated"
        );

        // The state's epoch should be updated
        assert_eq!(state.epoch(), curr_epoch + 1, "Epoch should be updated");

        assert!(
            *state.filter_config() != old_filter_config,
            "Filter config should be updated"
        );
    }

    #[tokio::test()]
    async fn test_fetch_timestamps() {
        let (bitcoind, client) = get_bitcoind_and_client();
        let _ = mine_blocks(&bitcoind, 115, None).unwrap();

        let ts = fetch_block_timestamps_ascending(&client, 15, 10)
            .await
            .unwrap();
        assert!(ts.is_sorted());

        let ts = fetch_block_timestamps_ascending(&client, 10, 10)
            .await
            .unwrap();
        assert!(ts.is_sorted());

        let ts = fetch_block_timestamps_ascending(&client, 5, 10)
            .await
            .unwrap();
        assert!(ts.is_sorted());
    }

    #[tokio::test()]
    async fn test_header_verification_state() {
        let (bitcoind, client) = get_bitcoind_and_client();
        let reorg_safe_depth = 5;

        let _ = mine_blocks(&bitcoind, 115, None).unwrap();

        let len = 2;
        let height = 100;
        let mut header_vs = get_verification_state(&client, height, &REGTEST, reorg_safe_depth)
            .await
            .unwrap();

        for h in height..height + len {
            let block = client.get_block_at(h).await.unwrap();
            header_vs
                .check_and_update_continuity(&block.header, &REGTEST)
                .unwrap();
        }

        let new_header_vs =
            get_verification_state(&client, height + len, &REGTEST, reorg_safe_depth)
                .await
                .unwrap();

        assert_eq!(
            header_vs.last_verified_block,
            new_header_vs.last_verified_block
        );
        assert_eq!(header_vs.next_block_target, new_header_vs.next_block_target);
        assert_eq!(header_vs.epoch_timestamps, new_header_vs.epoch_timestamps);
        assert_eq!(
            header_vs.block_timestamp_history,
            new_header_vs.block_timestamp_history
        );
        assert_eq!(
            header_vs.last_verified_block,
            new_header_vs.last_verified_block
        );
    }
}
