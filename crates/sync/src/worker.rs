use std::sync::Arc;

use futures::StreamExt;
use strata_consensus_logic::{
    csm::message::{ClientUpdateNotif, ForkChoiceMessage},
    sync_manager::SyncManager,
};
use strata_primitives::epoch::EpochCommitment;
use strata_state::{
    block::L2BlockBundle, client_state::SyncState, header::L2Header, id::L2BlockId,
};
use strata_storage::L2BlockManager;
use tracing::*;

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
    pub fn new(
        client: T,
        l2_block_manager: Arc<L2BlockManager>,
        sync_manager: Arc<SyncManager>,
    ) -> Self {
        Self {
            client,
            l2_block_manager,
            sync_manager,
        }
    }
}

/// Initialize the sync state from the database and wait for the CSM to become ready.
pub fn block_until_csm_ready_and_init_sync_state<T: SyncClient>(
    context: &L2SyncContext<T>,
) -> Result<L2SyncState, L2SyncError> {
    debug!("waiting for CSM to become ready");
    let sync_state = wait_for_csm_ready(&context.sync_manager);
    debug!(?sync_state, "CSM is ready");
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
                let Ok(update) = client_update else {
                    continue;
                };

                handle_new_client_update(update.as_ref(), state, context).await?;
            }

            _ = interval.tick() => {
                do_tick(state, context).await?;
            }
            // maybe subscribe to new blocks on client instead of polling?
        }
    }
}

async fn handle_new_client_update<T: SyncClient>(
    update: &ClientUpdateNotif,
    state: &mut L2SyncState,
    _context: &L2SyncContext<T>,
) -> Result<(), L2SyncError> {
    // on receiving new client update, update own finalized state

    let Some(sync) = update.new_state().sync() else {
        debug!("new state but chain hasn't started, ignoring");
        return Ok(());
    };

    let fin_epoch = *sync.finalized_epoch();
    let finalized_blkid = sync.finalized_blkid();

    // I think this can just be removed.
    /*

        let block = match context
            .l2_block_manager
            .get_block_data_async(finalized_blkid)
            .await
        {
            Ok(Some(block)) => block,

            Ok(None) => {
                // FIXME should we really just ignore it here?
                error!(%finalized_blkid, "missing newly finalized block, ignoring");
                return Ok(());
            }

            Err(e) => {
                // FIXME should we REALLY just ignore it here???
                error!(%finalized_blkid, err = %e, "error fetching finalized block, ignoring");
                return Ok(());
            }
        };
    */

    if let Err(e) = handle_block_finalized(state, fin_epoch).await {
        error!(%finalized_blkid, err = %e, "failed to handle newly finalized block");
    }

    Ok(())
}

async fn do_tick<T: SyncClient>(
    state: &mut L2SyncState,
    context: &L2SyncContext<T>,
) -> Result<(), L2SyncError> {
    // every fixed interval, try to sync with latest state of client
    let Ok(status) = context.client.get_sync_status().await else {
        // This should never *really* happen.
        warn!("failed to get client status");
        return Ok(());
    };

    if state.has_block(&status.tip_block_id) {
        // in sync with client
        return Ok(());
    }

    let start_slot = state.tip_height() + 1;
    let end_slot = status.tip_slot;

    let span = debug_span!("sync", %start_slot, %end_slot);

    /*debug!(
        current_height = state.tip_height(),
        target_height = status.tip_height,
        "syncing to target height"
    );*/

    if let Err(e) = sync_blocks_by_range(start_slot, end_slot, state, context)
        .instrument(span)
        .await
    {
        error!(%start_slot, %end_slot, err = ?e, "failed to make sync fetch");
    }

    Ok(())
}

async fn sync_blocks_by_range<T: SyncClient>(
    start_height: u64,
    end_height: u64,
    state: &mut L2SyncState,
    context: &L2SyncContext<T>,
) -> Result<(), L2SyncError> {
    debug!("syncing blocks by range");

    let block_stream = context.client.get_blocks_range(start_height, end_height);
    let mut block_stream = Box::pin(block_stream);

    while let Some(block) = block_stream.next().await {
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
            .put_block_data_async(block.clone())
            .await?;
        let block_idx = block.header().blockidx();
        debug!(%block_idx, "l2 sync: sending chain tip msg");
        context
            .sync_manager
            .submit_chain_tip_msg_async(ForkChoiceMessage::NewBlock(block.header().get_blockid()))
            .await;
        debug!(%block_idx, "l2 sync: sending chain tip sent");
    }
    Ok(())
}

async fn handle_block_finalized(
    state: &mut L2SyncState,
    new_finalized_epoch: EpochCommitment,
) -> Result<(), L2SyncError> {
    if state.finalized_blockid() == new_finalized_epoch.last_blkid() {
        return Ok(());
    }

    if !state.has_block(new_finalized_epoch.last_blkid()) {
        return Err(L2SyncError::MissingFinalized(
            *new_finalized_epoch.last_blkid(),
        ));
    };

    state.update_finalized_tip(new_finalized_epoch)?;

    Ok(())
}
