//! Extracts new duties for sequencer for a given consensus state.

use strata_db::types::CheckpointConfStatus;
use strata_primitives::params::Params;
use strata_state::{
    client_state::{ClientState, SyncState},
    header::L2Header,
};
use strata_storage::L2BlockManager;

use super::types::{BlockSigningDuty, Duty};
use crate::{
    checkpoint::CheckpointHandle,
    duty::{errors::Error, types::BatchCheckpointDuty},
};

/// Extracts new duties given a consensus state and a identity.
pub fn extract_duties(
    state: &ClientState,
    checkpoint_handle: &CheckpointHandle,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<Vec<Duty>, Error> {
    // If a sync state isn't present then we probably don't have anything we
    // want to do.  We might change this later.
    let Some(ss) = state.sync() else {
        return Ok(Vec::new());
    };

    let mut duties = vec![];

    duties.extend(extract_block_duties(ss, l2_block_manager, params)?);
    duties.extend(extract_batch_duties(checkpoint_handle)?);

    Ok(duties)
}

fn extract_block_duties(
    ss: &SyncState,
    l2_block_manager: &L2BlockManager,
    params: &Params,
) -> Result<Vec<Duty>, Error> {
    let tip_height = ss.chain_tip_height();
    let tip_blkid = *ss.chain_tip_blkid();

    let tip_block_ts = l2_block_manager
        .get_block_data_blocking(&tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?
        .header()
        .timestamp();

    let target_ts = tip_block_ts + params.rollup().block_time;

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    Ok(vec![Duty::SignBlock(BlockSigningDuty::new_simple(
        tip_height + 1,
        tip_blkid,
        target_ts,
    ))])
}

fn extract_batch_duties(checkpoint_handle: &CheckpointHandle) -> Result<Vec<Duty>, Error> {
    // get checkpoints ready to be signed
    let last_checkpoint_idx = checkpoint_handle.get_last_checkpoint_idx_blocking()?;

    let last_checkpoint = last_checkpoint_idx
        .map(|idx| checkpoint_handle.get_checkpoint_blocking(idx))
        .transpose()?
        .flatten();

    last_checkpoint
        .filter(|entry| {
            entry.is_proof_ready() && entry.confirmation_status == CheckpointConfStatus::Pending
        })
        .map(|entry| {
            let batch_duty = BatchCheckpointDuty::new(entry.into());
            Ok(vec![Duty::CommitBatch(batch_duty)])
        })
        .unwrap_or(Ok(vec![]))
}
