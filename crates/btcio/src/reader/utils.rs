use strata_db::traits::BlockStatus;
use strata_l1tx::messages::{L1Event, RelevantTxEntry};
use strata_primitives::l1::ProtocolOperation;
use strata_state::batch::SignedCheckpoint;
use strata_storage::{L1BlockManager, NodeStorage};
use tracing::*;

/// Looks for checkpoints in given `RelevantTxEntry`s.
pub(crate) fn find_checkpoint(entries: &[RelevantTxEntry]) -> Option<&SignedCheckpoint> {
    let checkpts: Vec<&SignedCheckpoint> = entries
        .iter()
        .flat_map(|txentry| {
            txentry
                .contents()
                .protocol_ops()
                .iter()
                .filter_map(|op| match op {
                    ProtocolOperation::Checkpoint(c) => Some(c),
                    _ => None,
                })
        })
        .collect();
    checkpts.last().map(|v| &**v)
}

pub(crate) async fn wait_for_terminal_block_in_db(
    storage: &NodeStorage,
    chkpt: &SignedCheckpoint,
) -> anyhow::Result<()> {
    let l2blk_comm = chkpt.checkpoint().batch_info().final_l2_block();
    let exp_blkid = l2blk_comm.blkid();
    let exp_height = l2blk_comm.slot();
    let l2mgr = storage.l2();

    if l2mgr.get_block_data_async(exp_blkid).await?.is_some() {
        // Return if valid, else wait for valid block or some other block at the height is valid.
        if let Some(BlockStatus::Valid) = l2mgr.get_block_status_async(exp_blkid).await? {
            return Ok(());
        }
    }
    debug!(
        ?l2blk_comm,
        "Found checkpoint in L1 block, waiting for corresponding processed terminal L2 block"
    );

    let mut rx = l2mgr.subscribe_to_valid_block_updates();

    // FIXME: Ideally we would wait for the valid block until some timeout because that will
    // never be seen if L2 reorgs.
    // Or, Instead for waiting on particular block id, ideally we would wait for canonical block
    // at given height and see if we get different blockid than we expect. That would
    // handle for reorgs too.
    loop {
        match rx.recv().await? {
            (h, blkid) if h >= exp_height || blkid == *exp_blkid => {
                debug!(
                    ?h,
                    ?blkid,
                    "Found a block at given height in db, continuing..."
                );
                return Ok(());
            }
            (h, blkid) => {
                debug!(%h, %blkid, "Received valid block update");
            }
        }
    }
}

/// Looks for checkpoints in given L1 events
pub(crate) fn find_checkpoint_in_events(evs: &[L1Event]) -> Option<&SignedCheckpoint> {
    for ev in evs {
        if let L1Event::BlockData(blk_data, _, _) = ev {
            if let Some(checkpt) = find_checkpoint(blk_data.relevant_txs()) {
                return Some(checkpt);
            }
        }
    }
    None
}

/// Given a L1 height, fetches corresponding manifest and if manifest exists and it has checkpoint
/// in it, returns the checkpoint.
/// This is the checkpoint whose terminal l2 block will be waited to be present in the database.
pub(crate) fn find_initial_checkpoint_to_wait_for(
    l1mgr: &L1BlockManager,
    last_block: u64,
) -> anyhow::Result<Option<SignedCheckpoint>> {
    let checkpt = l1mgr
        .get_block_manifest_at_height(last_block)?
        .map(|bmf| {
            bmf.txs()
                .iter()
                .flat_map(|tx| {
                    tx.protocol_ops().iter().filter_map(|op| match op {
                        ProtocolOperation::Checkpoint(c) => Some(c.clone()),
                        _ => None,
                    })
                })
                .collect::<Vec<_>>()
        })
        .and_then(|v| v.last().cloned());
    Ok(checkpt)
}
