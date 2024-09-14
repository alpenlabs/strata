use alpen_express_primitives::params::Params;
use alpen_express_state::{batch::CheckPointInfo, client_state::ClientState, id::L2BlockId};
use tracing::*;

use super::types::{BatchCheckpointDuty, BlockSigningDuty, Duty, Identity};
use crate::errors::Error;

/// Extracts new duties given a consensus state and a identity.
pub fn extract_duties(
    state: &ClientState,
    _ident: &Identity,
    _params: &Params,
) -> Result<Vec<Duty>, Error> {
    // If a sync state isn't present then we probably don't have anything we
    // want to do.  We might change this later.
    let Some(ss) = state.sync() else {
        return Ok(Vec::new());
    };

    let tip_height = ss.chain_tip_height();
    let tip_blkid = *ss.chain_tip_blkid();

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    let duty_data = BlockSigningDuty::new_simple(tip_height + 1, tip_blkid);
    let mut duties = vec![Duty::SignBlock(duty_data)];

    duties.extend(extract_batch_duties(state, tip_height, tip_blkid)?);

    Ok(duties)
}

fn extract_batch_duties(
    state: &ClientState,
    tip_height: u64,
    tip_id: L2BlockId,
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
            let first_batch_idx = 1;

            // Include genesis l1 height to current seen height
            let l1_range = state.genesis_l1_height()..=state.l1_view().tip_height();

            // Start from first non-genesis l2 block height
            let l2_range = 1..=tip_height;

            let new_checkpt = CheckPointInfo::new(first_batch_idx, l1_range, l2_range, tip_id);
            Ok(vec![Duty::CommitBatch(new_checkpt.clone().into())])
        }
        Some(checkpt_state) => {
            let checkpoint = checkpt_state.checkpoint.clone();
            let l1_range = checkpoint.l1_range.end() + 1..=state.l1_view().tip_height();
            let l2_range = checkpoint.l2_range.end() + 1..=tip_height;
            let new_checkpt = CheckPointInfo::new(checkpoint.idx + 1, l1_range, l2_range, tip_id);
            Ok(vec![Duty::CommitBatch(new_checkpt.clone().into())])
        }
    }
}
