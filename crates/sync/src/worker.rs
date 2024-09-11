use std::sync::Arc;

use alpen_express_consensus_logic::{message::ForkChoiceMessage, sync_manager::SyncManager};
use alpen_express_state::{
    block::L2BlockBundle, client_state::SyncState, header::L2Header, id::L2BlockId,
};
use express_storage::L2BlockManager;
use futures::StreamExt;
use tracing::{debug, error, warn};

use crate::{
    state::{self, L2SyncState},
    L2SyncError, SyncClient,
};

pub struct L2SyncContext<T: SyncClient> {
    client: T,
    l2_block_manager: Arc<L2BlockManager>,
    sync_manager: Arc<SyncManager>,
}

impl<T: SyncClient> L2SyncContext<T> {
    pub fn new(client: T, l2_manager: Arc<L2BlockManager>, sync_man: Arc<SyncManager>) -> Self {
        Self {
            client,
            l2_block_manager: l2_manager,
            sync_manager: sync_man,
        }
    }
}

/// Initialize the sync state from the database and wait for the CSM to become ready.
pub fn block_until_csm_ready_and_init_sync_state<T: SyncClient>(
    context: &L2SyncContext<T>,
) -> Result<L2SyncState, L2SyncError> {
    debug!("waiting for CSM to become ready");
    let sync_state = wait_for_csm_ready(&context.sync_manager);

    debug!(sync_state = ?sync_state, "CSM is ready");

    state::initialize_from_db(&sync_state, &context.l2_block_manager)
}

fn wait_for_csm_ready(sync_man: &SyncManager) -> SyncState {
    let mut client_update_notif = sync_man.create_cstate_subscription();

    loop {
        let Ok(update) = client_update_notif.blocking_recv() else {
            continue;
        };
        let state = update.new_state();
        if state.is_chain_active() && state.sync().is_some() {
            return state.sync().unwrap().clone();
        }
    }
}

pub async fn sync_worker<T: SyncClient>(
    state: &mut L2SyncState,
    context: &L2SyncContext<T>,
) -> Result<(), L2SyncError> {
    let mut client_update_notif = context.sync_manager.create_cstate_subscription();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            client_update = client_update_notif.recv() => {
                // on receiving new client update, update own finalized state
                let Ok(update) = client_update else {
                    continue;
                };
                let Some(sync) = update.new_state().sync() else {
                    continue;
                };

                let finalized_blockid = sync.finalized_blkid();
                let Ok(Some(finalized_block)) = context.l2_block_manager.get_block_async(finalized_blockid).await else {
                    error!("missing finalized block {}", finalized_blockid);
                    continue;
                };
                let finalized_height = finalized_block.header().blockidx();
                if let Err(err) = handle_block_finalized(state, finalized_blockid, finalized_height).await {
                    error!("failed to finalize block {}: {err}", sync.finalized_blkid());
                }
            }
            _ = interval.tick() => {
                // every fixed interval, try to sync with latest state of client
                let Ok(status) = context.client.get_sync_status().await else {
                    warn!("failed to get client status");
                    continue;
                };

                if state.has_block(&status.tip_block_id) {
                    // in sync with client
                    continue;
                }

                debug!(current_height = state.tip_height(), target_height = status.tip_height, "syncing to target height");

                let start_height = state.tip_height() + 1;
                let end_height = status.tip_height;

                if let Err(err) = sync_blocks_by_range(state, context, start_height, end_height).await {
                    error!(start_height = start_height, end_height = end_height, err = ?err, "failed to sync blocks");
                }
            }
            // maybe subscribe to new blocks on client instead of polling?
        }
    }
}

async fn sync_blocks_by_range<T: SyncClient>(
    state: &mut L2SyncState,
    context: &L2SyncContext<T>,
    start_height: u64,
    end_height: u64,
) -> Result<(), L2SyncError> {
    debug!(
        start_height = start_height,
        end_height = end_height,
        "syncing blocks by range"
    );

    let blockstream = context.client.get_blocks_range(start_height, end_height);
    let mut blockstream = Box::pin(blockstream);

    while let Some(block) = blockstream.next().await {
        handle_new_block(state, context, block).await?;
    }

    Ok(())
}

/// Process a new block received from the client.
///
/// The block is added to the unfinalized chain and the corresponding
/// messages are submitted to the fork choice manager.
///
/// If the parent block is missing, it will be fetched recursively
/// until we reach a known block in our unfinalized chain.
async fn handle_new_block<T: SyncClient>(
    state: &mut L2SyncState,
    context: &L2SyncContext<T>,
    block: L2BlockBundle,
) -> Result<(), L2SyncError> {
    let mut block = block;
    let mut fetched_blocks = vec![];

    loop {
        let block_id = block.header().get_blockid();
        debug!(block_id = ?block_id, height = block.header().blockidx(), "received new block");

        if state.has_block(&block_id) {
            warn!(block_id = ?block_id, "block already known");
            return Ok(());
        }

        let height = block.header().blockidx();
        if height <= state.finalized_height() {
            // got block on different fork than one we're finalized on
            // log error and ignore received blocks
            error!(height = height, block_id = ?block_id, "got block on incompatible fork");
            return Err(L2SyncError::WrongFork(block_id, height));
        }

        let parent_block_id = block.header().parent();

        fetched_blocks.push(block.clone());

        if state.has_block(parent_block_id) {
            break;
        }

        // parent block does not exist in our unfinalized chain
        // try to fetch it and continue
        let Some(parent_block) = context.client.get_block_by_id(parent_block_id).await? else {
            // block not found
            error!("parent block {parent_block_id} not found");
            return Err(L2SyncError::MissingBlock(*parent_block_id));
        };

        block = parent_block;
    }

    // send ForkChoiceMessage::NewBlock for all pending blocks in correct order
    while let Some(block) = fetched_blocks.pop() {
        state.attach_block(block.header())?;
        context
            .l2_block_manager
            .put_block_async(block.clone())
            .await?;
        context
            .sync_manager
            .submit_chain_tip_msg_async(ForkChoiceMessage::NewBlock(block.header().get_blockid()))
            .await;
    }
    Ok(())
}

async fn handle_block_finalized(
    state: &mut L2SyncState,
    new_finalized_blockid: &L2BlockId,
    new_finalized_height: u64,
) -> Result<(), L2SyncError> {
    if state.finalized_blockid() == new_finalized_blockid {
        return Ok(());
    }

    if !state.has_block(new_finalized_blockid) {
        return Err(L2SyncError::MissingFinalized(*new_finalized_blockid));
    };

    state.update_finalized_tip(new_finalized_blockid, new_finalized_height)?;

    Ok(())
}
