use alpen_vertex_db::traits::{Database, L2DataProvider};
use alpen_vertex_state::consensus::ConsensusState;

use crate::{
    duties::{self, BlockSigningDuty},
    errors::Error,
};

/// Extracts new duties given a consensus state and a identity.
pub fn extract_duties<D: Database>(
    state: &ConsensusState,
    _ident: &duties::Identity,
    database: &D,
) -> Result<Vec<duties::Duty>, Error> {
    let cstate = state.chain_state();
    let tip_blkid = cstate.chain_tip_blockid();

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let l2prov = database.l2_provider();
    let block = l2prov
        .get_block_data(tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    let duty_data = BlockSigningDuty::new_simple(block_idx + 1);
    Ok(vec![duties::Duty::SignBlock(duty_data)])
}
