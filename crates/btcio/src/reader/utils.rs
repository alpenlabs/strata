use strata_l1tx::messages::{L1Event, RelevantTxEntry};
use strata_primitives::{epoch::EpochCommitment, l1::ProtocolOperation};
use strata_state::{batch::SignedCheckpoint, chain_state::Chainstate};
use strata_storage::NodeStorage;
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
pub(crate) async fn find_last_checkpoint_chainstate(
    storage: &NodeStorage,
) -> anyhow::Result<Option<Chainstate>> {
    let Some((_, client_state)) = storage.client_state().get_most_recent_state().await else {
        return Ok(None);
    };
    let Some(last_checkpoint) = client_state.get_last_checkpoint() else {
        return Ok(None);
    };
    let Some(last_checkpoint_block) = storage
        .l1()
        .get_block_manifest_async(&last_checkpoint.l1_reference.blockid)
        .await?
    else {
        return Ok(None);
    };

    let chainstate = last_checkpoint_block
        .txs()
        .iter()
        .flat_map(|tx| tx.protocol_ops())
        .find_map(|op| match op {
            ProtocolOperation::Checkpoint(c) => {
                let chainstate: Chainstate =
                    borsh::from_slice(c.checkpoint().sidecar().chainstate()).unwrap();
                Some(chainstate)
            }
            _ => None,
        });

    Ok(chainstate)
}
