use strata_l1tx::messages::{L1Event, RelevantTxEntry};
use strata_primitives::{epoch::EpochCommitment, l1::ProtocolOperation};
use strata_state::{batch::SignedCheckpoint, chain_state::Chainstate};
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

pub(crate) async fn get_or_wait_for_chainstate(
    storage: &NodeStorage,
    epoch: EpochCommitment,
) -> anyhow::Result<Chainstate> {
    let chainstate_manager = storage.chainstate();
    let slot = epoch.last_slot();

    if let Some(chainstate) = chainstate_manager
        .get_toplevel_chainstate_async(slot)
        .await?
    {
        return Ok(chainstate);
    }

    debug!(?epoch, "epoch chainstate not found, waiting");

    let mut rx = chainstate_manager.subscribe_chainstate_updates();

    loop {
        match rx.recv().await? {
            idx if idx >= slot => {
                debug!(
                    %idx,
                    %slot,
                    "Found a chainstate at or above required slot"
                );
                let chainstate = chainstate_manager
                    .get_toplevel_chainstate_async(slot)
                    .await?
                    .expect("chainstate should be found in db");

                return Ok(chainstate);
            }
            _ => {}
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
