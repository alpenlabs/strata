use strata_db::traits::ChainstateDatabase;
use strata_primitives::{buf::Buf32, params::Params};
use strata_state::{batch::BatchInfo, client_state::ClientState, id::L2BlockId};
use tracing::*;

use super::types::{BlockSigningDuty, Duty, Identity};
use crate::{duty::types::BatchCheckpointDuty, errors::Error};

/// Extracts new duties given a consensus state and a identity.
pub fn extract_duties(
    state: &ClientState,
    _ident: &Identity,
    _params: &Params,
    chs_db: &impl ChainstateDatabase,
    rollup_params_commitment: Buf32,
) -> Result<Vec<Duty>, Error> {
    // If a sync state isn't present then we probably don't have anything we
    // want to do.  We might change this later.
    let Some(ss) = state.sync() else {
        return Ok(Vec::new());
    };

    let tip_height = ss.tip_height();
    let tip_blkid = *ss.chain_tip_blkid();

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    let duty_data = BlockSigningDuty::new_simple(tip_height + 1, tip_blkid);
    let mut duties = vec![Duty::SignBlock(duty_data)];

    duties.extend(extract_batch_duties(
        state,
        tip_height,
        tip_blkid,
        chs_db,
        rollup_params_commitment,
    )?);

    Ok(duties)
}

fn extract_batch_duties(
    state: &ClientState,
    tip_height: u64,
    tip_id: L2BlockId,
    chs_db: &impl ChainstateDatabase,
    rollup_params_commitment: Buf32,
) -> Result<Vec<Duty>, Error> {
    if !state.is_chain_active() {
        debug!("chain not active, no duties created");
        // There are no duties if the chain is not yet active
        return Ok(vec![]);
    };

    match state.l1_view().last_finalized_checkpoint() {
        // Cool, we are producing first batch!
        None => {
            debug!(
                ?tip_height,
                ?tip_id,
                "No finalized checkpoint, creating new checkpiont"
            );
            // But wait until we've move past genesis, perhaps this can be
            // configurable. Right now this is not ideal because we will be wasting proving resource
            // just for a couple of initial blocks in the first batch
            if tip_height == 0 {
                return Ok(vec![]);
            }
            let first_checkpoint_idx = 0;

            // Include genesis l1 height to current seen height
            let l1_range = (
                state.genesis_l1_height(),
                state.l1_view().tip_l1_block_height(),
            );

            let genesis_l1_state_hash = state
                .genesis_verification_hash()
                .ok_or(Error::ChainInactive)?;

            let current_l1_state = state
                .l1_view()
                .tip_verification_state()
                .ok_or(Error::ChainInactive)?;

            // Include blocks after genesis l1 height to last verified height
            let l1_range = (
                state.genesis_l1_height() + 1,
                current_l1_state.last_verified_block_num as u64,
            );

            let genesis_l1_state_hash = state
                .genesis_verification_hash()
                .ok_or(Error::ChainInactive)?;
            let current_l1_state_hash = current_l1_state.compute_hash().unwrap();
            let l1_transition = (genesis_l1_state_hash, current_l1_state_hash);

            // Start from first non-genesis l2 block height
            let l2_range = (1, tip_height);

            let initial_chain_state = chs_db
                .get_toplevel_state(0)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let initial_chain_state_root = initial_chain_state.compute_state_root();

            let current_chain_state = chs_db
                .get_toplevel_state(tip_height)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (initial_chain_state_root, current_chain_state_root);

            let new_batch = BatchInfo::new(
                first_checkpoint_idx,
                l1_range,
                l2_range,
                l1_transition,
                l2_transition,
                tip_id,
                (0, current_l1_state.total_accumulated_pow),
                rollup_params_commitment,
            );

            let genesis_bootstrap = new_batch.get_initial_bootstrap_state();
            let batch_duty = BatchCheckpointDuty::new(new_batch, genesis_bootstrap);
            Ok(vec![Duty::CommitBatch(batch_duty)])
        }
        Some(prev_checkpoint) => {
            let checkpoint = prev_checkpoint.batch_info.clone();

            let l1_range = (
                checkpoint.l1_range.1 + 1,
                state.l1_view().tip_l1_block_height(),
            );

            let current_l1_state = state
                .l1_view()
                .tip_verification_state()
                .ok_or(Error::ChainInactive)?;

            let current_l1_state_hash = current_l1_state.compute_hash().unwrap();
            let l1_transition = (checkpoint.l1_transition.1, current_l1_state_hash);

            // Also, rather than tip heights, we might need to limit the max range a prover will be
            // proving
            let l2_range = (checkpoint.l2_range.1 + 1, tip_height);
            let current_chain_state = chs_db
                .get_toplevel_state(tip_height)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (checkpoint.l2_transition.1, current_chain_state_root);

            let new_batch = BatchInfo::new(
                checkpoint.epoch + 1,
                l1_range,
                l2_range,
                l1_transition,
                l2_transition,
                tip_id,
                (
                    checkpoint.l1_pow_transition.1,
                    current_l1_state.total_accumulated_pow,
                ),
                rollup_params_commitment,
            );

            // If prev checkpoint was proved, use the bootstrap state of the prev checkpoint
            // else create a bootstrap state based on initial info of this batch
            let bootstrap_state = if prev_checkpoint.is_proved {
                prev_checkpoint.bootstrap_state.clone()
            } else {
                new_batch.get_initial_bootstrap_state()
            };
            let batch_duty = BatchCheckpointDuty::new(new_batch, bootstrap_state);
            Ok(vec![Duty::CommitBatch(batch_duty)])
        }
    }
}
