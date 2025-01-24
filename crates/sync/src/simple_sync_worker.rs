use futures::StreamExt;
use strata_consensus_logic::message::ForkChoiceMessage;
use strata_state::{block::L2BlockBundle, header::L2Header};
use tracing::{debug, error, info, warn};

use crate::{L2SyncContext, L2SyncError, SyncClient};

/// Dumb version of sync worker that does not handle forks
pub async fn simple_sync_worker<T: SyncClient>(
    context: &L2SyncContext<T>,
) -> Result<(), L2SyncError> {
    let cl_rx = context.sync_manager.status_tx().cl.subscribe();

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        interval.tick().await;

        let sync_state = {
            let cs = cl_rx.borrow();
            if !cs.is_chain_active() {
                None
            } else {
                cs.sync().cloned()
            }
        };
        let Some(sync_state) = sync_state else {
            warn!("chain inactive or no sync state; cannot sync yet");
            continue;
        };

        let Ok(peer_status) = context.client.get_sync_status().await else {
            warn!("failed to get client status");
            continue;
        };

        if sync_state.chain_tip_height() >= peer_status.tip_height {
            info!("in sync");
            continue;
        }

        let start_height = sync_state.chain_tip_height() + 1;
        let end_height = peer_status.tip_height;

        if let Err(err) = sync_blocks_by_range(context, start_height, end_height).await {
            error!(start_height = start_height, end_height = end_height, err = ?err, "failed to sync blocks");
        }
    }
}

async fn sync_blocks_by_range<T: SyncClient>(
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
        handle_new_block(context, block).await?;
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
    context: &L2SyncContext<T>,
    block: L2BlockBundle,
) -> Result<(), L2SyncError> {
    let block_idx = block.header().blockidx();
    let block_id = block.header().get_blockid();
    context.l2_block_manager.put_block_async(block).await?;
    debug!(%block_idx, "l2 sync: sending chain tip msg");
    context
        .sync_manager
        .submit_chain_tip_msg_async(ForkChoiceMessage::NewBlock(block_id))
        .await;
    debug!(%block_idx, "l2 sync: sending chain tip sent");

    Ok(())
}
