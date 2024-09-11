use alpen_express_db::traits::{Database, L2DataProvider};
use alpen_express_primitives::params::Params;
use alpen_express_state::{
    batch::CheckPoint, block::L2BlockBundle, client_state::ClientState, header::L2Header,
};

use super::types::{BatchCommitmentDuty, BlockSigningDuty, Duty, Identity};
use crate::errors::Error;

/// Extracts new duties given a consensus state and a identity.
pub fn extract_duties<D: Database>(
    state: &ClientState,
    _ident: &Identity,
    database: &D,
    _params: &Params,
) -> Result<Vec<Duty>, Error> {
    // If a sync state isn't present then we probably don't have anything we
    // want to do.  We might change this later.
    let Some(ss) = state.sync() else {
        return Ok(Vec::new());
    };

    let tip_blkid = *ss.chain_tip_blkid();

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let l2prov = database.l2_provider();
    let block = l2prov
        .get_block_data(tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    let duty_data = BlockSigningDuty::new_simple(block_idx + 1, tip_blkid);
    let mut duties = vec![Duty::SignBlock(duty_data)];

    duties.extend(extract_batch_duties(state, block)?);

    Ok(duties)
}

fn extract_batch_duties(state: &ClientState, tip: L2BlockBundle) -> Result<Vec<Duty>, Error> {
    if state.sync().is_none() {
        return Err(Error::MissingClientSyncState);
    };

    match state.l1_view().last_finalized_checkpoint() {
        // No checkpoint is seen, start from 0
        None => {
            let new_checkpt = CheckPoint::new(
                0,
                0..=state.l1_view().tip_height(),
                0..=tip.header().blockidx(),
                tip.header().get_blockid(),
            );
            Ok(vec![Duty::CommitBatch(new_checkpt.clone().into())])
        }
        Some(checkpt_state) => {
            let checkpoint = checkpt_state.checkpoint.clone();
            let l1_range = checkpoint.l1_range.end() + 1..=state.l1_view().tip_height();
            let l2_range = checkpoint.l2_range.end() + 1..=tip.header().blockidx();
            let new_checkpt = CheckPoint::new(
                checkpoint.checkpoint_idx + 1,
                l1_range,
                l2_range,
                tip.header().get_blockid(),
            );
            let duty: BatchCommitmentDuty = new_checkpt.clone().into();
            Ok(vec![Duty::CommitBatch(duty)])
        }
    }
}
