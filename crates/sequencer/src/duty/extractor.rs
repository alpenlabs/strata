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

// fn _extract_batch_duties(
//     state: &ClientState,
//     tip_height: u64,
//     tip_id: L2BlockId,
//     chs_db: &impl ChainstateDatabase,
//     rollup_params_commitment: Buf32,
// ) -> Result<Vec<Duty>, Error> {
//     if !state.is_chain_active() {
//         debug!("chain not active, no duties created");
//         // There are no duties if the chain is not yet active
//         return Ok(vec![]);
//     };

//     match state.l1_view().last_finalized_checkpoint() {
//         // Cool, we are producing first batch!
//         None => {
//             debug!(
//                 ?tip_height,
//                 ?tip_id,
//                 "No finalized checkpoint, creating new checkpiont"
//             );
//             // But wait until we've move past genesis, perhaps this can be
//             // configurable. Right now this is not ideal because we will be wasting proving
// resource             // just for a couple of initial blocks in the first batch
//             if tip_height == 0 {
//                 return Ok(vec![]);
//             }
//             let first_checkpoint_idx = 0;

//             // Include genesis l1 height to current seen height
//             let l1_range = (state.genesis_l1_height(), state.l1_view().tip_height());

//             let genesis_l1_state_hash = state
//                 .genesis_verification_hash()
//                 .ok_or(Error::ChainInactive)?;
//             let current_l1_state = state
//                 .l1_view()
//                 .tip_verification_state()
//                 .ok_or(Error::ChainInactive)?;
//             let current_l1_state_hash = current_l1_state.compute_hash().unwrap();
//             let l1_transition = (genesis_l1_state_hash, current_l1_state_hash);

//             // Start from first non-genesis l2 block height
//             let l2_range = (1, tip_height);

//             let initial_chain_state = chs_db
//                 .get_toplevel_state(0)?
//                 .ok_or(Error::MissingIdxChainstate(0))?;
//             let initial_chain_state_root = initial_chain_state.compute_state_root();

//             let current_chain_state = chs_db
//                 .get_toplevel_state(tip_height)?
//                 .ok_or(Error::MissingIdxChainstate(0))?;
//             let current_chain_state_root = current_chain_state.compute_state_root();
//             let l2_transition = (initial_chain_state_root, current_chain_state_root);

//             let new_batch = BatchInfo::new(
//                 first_checkpoint_idx,
//                 l1_range,
//                 l2_range,
//                 l1_transition,
//                 l2_transition,
//                 tip_id,
//                 (0, current_l1_state.total_accumulated_pow),
//                 rollup_params_commitment,
//             );

//             let genesis_bootstrap = new_batch.get_initial_bootstrap_state();
//             let batch_duty = BatchCheckpointDuty::new(new_batch, genesis_bootstrap);
//             Ok(vec![Duty::CommitBatch(batch_duty)])
//         }
//         Some(prev_checkpoint) => {
//             let checkpoint = prev_checkpoint.batch_info.clone();

//             let l1_range = (checkpoint.l1_range.1 + 1, state.l1_view().tip_height());
//             let current_l1_state = state
//                 .l1_view()
//                 .tip_verification_state()
//                 .ok_or(Error::ChainInactive)?;
//             let current_l1_state_hash = current_l1_state.compute_hash().unwrap();
//             let l1_transition = (checkpoint.l1_transition.1, current_l1_state_hash);

//             // Also, rather than tip heights, we might need to limit the max range a prover will
// be             // proving
//             let l2_range = (checkpoint.l2_range.1 + 1, tip_height);
//             let current_chain_state = chs_db
//                 .get_toplevel_state(tip_height)?
//                 .ok_or(Error::MissingIdxChainstate(0))?;
//             let current_chain_state_root = current_chain_state.compute_state_root();
//             let l2_transition = (checkpoint.l2_transition.1, current_chain_state_root);

//             let new_batch = BatchInfo::new(
//                 checkpoint.idx + 1,
//                 l1_range,
//                 l2_range,
//                 l1_transition,
//                 l2_transition,
//                 tip_id,
//                 (
//                     checkpoint.l1_pow_transition.1,
//                     current_l1_state.total_accumulated_pow,
//                 ),
//                 rollup_params_commitment,
//             );

//             // If prev checkpoint was proved, use the bootstrap state of the prev checkpoint
//             // else create a bootstrap state based on initial info of this batch
//             let bootstrap_state = if prev_checkpoint.is_proved {
//                 prev_checkpoint.bootstrap_state.clone()
//             } else {
//                 new_batch.get_initial_bootstrap_state()
//             };
//             let batch_duty = BatchCheckpointDuty::new(new_batch, bootstrap_state);
//             Ok(vec![Duty::CommitBatch(batch_duty)])
//         }
//     }
// }