//! Extracts new duties for sequencer for a given consensus state.

use strata_db::types::CheckpointConfStatus;
use strata_primitives::params::Params;
use strata_state::{chain_state::Chainstate, client_state::InternalState, header::L2Header};
use strata_storage::L2BlockManager;
use tracing::*;

use super::types::{BlockSigningDuty, Duty};
use crate::{
    checkpoint::CheckpointHandle,
    duty::{errors::Error, types::CheckpointDuty},
};

/// Extracts new duties given a current chainstate and an identity.
pub(crate) fn extract_duties(
    chstate: &Chainstate,
    cistate: &InternalState,
    checkpoint_handle: &CheckpointHandle,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<Vec<Duty>, Error> {
    let mut duties = vec![];
    duties.extend(extract_block_duties(chstate, l2_block_manager, params)?);
    duties.extend(extract_batch_duties(cistate, checkpoint_handle)?);

    if !duties.is_empty() {
        debug!(cnt = %duties.len(), "have some duties");
    }

    Ok(duties)
}

fn extract_block_duties(
    state: &Chainstate,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<Vec<Duty>, Error> {
    let tip_slot = state.chain_tip_slot();
    let tip_blkid = *state.chain_tip_blkid();

    let tip_block_ts = l2_block_manager
        .get_block_data_blocking(&tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?
        .header()
        .timestamp();

    let target_ts = tip_block_ts + params.rollup().block_time;

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    Ok(vec![Duty::SignBlock(BlockSigningDuty::new_simple(
        tip_slot + 1,
        tip_blkid,
        target_ts,
    ))])
}

fn extract_batch_duties(
    cistate: &InternalState,
    checkpoint_handle: &CheckpointHandle,
) -> Result<Vec<Duty>, Error> {
    // Get the next epoch we expect to be confirmed and start looking there.
    let first_epoch_idx = cistate.get_next_expected_epoch_conf();

    // get checkpoints ready to be signed
    let Some(last_checkpoint_idx) = checkpoint_handle.get_last_checkpoint_idx_blocking()? else {
        // No checkpoints generated yet, nothing to publish.
        return Ok(Vec::new());
    };

    let mut duties = Vec::new();

    for i in first_epoch_idx..=last_checkpoint_idx {
        let Some(ckpt) = checkpoint_handle.get_checkpoint_blocking(i)? else {
            error!(ckpt = %i, "database told us we had checkpoint but it was missing, moving on");
            break;
        };

        let epoch = ckpt.checkpoint.batch_info().epoch;
        let publish_ready =
            ckpt.is_proof_ready() && ckpt.confirmation_status == CheckpointConfStatus::Pending;
        trace!(%epoch, %publish_ready, "considering generating checkpoint publish duty");

        // Need to wait for a proof.  Also avoid generating a duty if it's already in the pipe
        if publish_ready {
            let duty = CheckpointDuty::new(ckpt.into());
            duties.push(Duty::CommitBatch(duty));
        }
    }

    Ok(duties)
}
