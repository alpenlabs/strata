use std::sync::Arc;

use strata_db::traits::*;
use strata_primitives::l2::L2BlockId;
use strata_state::{block::L2BlockBundle, fcm_state::FcmState};
use strata_status::StatusChannel;
use tokio::sync::broadcast;
use tracing::*;

use crate::{duty::types::Identity, errors::Error};

pub fn duty_worker_task(status_ch: Arc<StatusChannel>, db: Arc<impl Database>) {
    let fcm_rx = status_ch.subscribe_fcm_state();

    if let Err(e) = duty_worker_task_inner(fcm_rx, db.as_ref()) {
        error!(err = %e, "duty worker task failed");
    }
}

fn duty_worker_task_inner(
    mut fcm_rx: broadcast::Receiver<FcmState>,
    ident: Identity,
    db: &impl Database,
) -> anyhow::Result<()> {
    loop {
        let new_state = match fcm_rx.blocking_recv() {
            Ok(st) => st,
            // TODO make this handle errors properly
            Err(e) => todo!(),
        };

        handle_new_state(&new_state, db)?;
    }

    Ok(())
}

fn handle_new_state(
    new_state: &FcmState,
    ident: Identity,
    db: &impl Database,
) -> anyhow::Result<()> {
    let block_db = db.l2_db();

    let tip_blkid = *new_state.tip().blkid();
    let tip_block = fetch_block(&tip_blkid, block_db.as_ref())?;
    let parent_blkid = tip_blkid; // TODO make this actually the parent
    let parent_block = fetch_block(&parent_blkid, block_db.as_ref())?;

    Ok(())
}

/// Fetches a block from the database, propagating an error for missing blocks.
fn fetch_block(blkid: &L2BlockId, db: &impl L2BlockDatabase) -> anyhow::Result<L2BlockBundle> {
    // TODO add .with_context to be nice here
    let block = db
        .get_block_data(*blkid)?
        .ok_or(Error::MissingL2Block(*blkid))?;
    Ok(block)
}
