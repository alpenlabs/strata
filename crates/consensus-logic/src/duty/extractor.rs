use alpen_express_db::traits::{Database, L2DataProvider};
use alpen_express_primitives::params::Params;
use alpen_express_state::{client_state::ClientState, header::L2Header};

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

    duties.push(extract_batch_duty(state)?);

    Ok(duties)
}

fn extract_batch_duty(state: &ClientState) -> Result<Duty, Error> {
    if state.sync().is_none() {
        return Err(Error::MissingClientSyncState);
    };

    let checkpoint_info = state
        .l1_view()
        .next_checkpoint_info()
        .expect("expect checkpoint info to be present");
    let duty: BatchCommitmentDuty = checkpoint_info.clone().into();
    Ok(Duty::CommitBatch(duty))
}
