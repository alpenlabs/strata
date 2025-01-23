//! Duty extraction logic.

use std::sync::Arc;

use strata_db::traits::*;
use strata_primitives::prelude::*;
use strata_storage::L2BlockManager;
use tracing::*;

use super::{errors::*, types::*};
use crate::duty::types::Identity;

pub fn get_duties_for_slot(
    slot: u64,
    ident: Identity,
    l2blkman: Arc<L2BlockManager>,
    db: &impl Database,
) -> anyhow::Result<Vec<SlotDuty>> {
    let chs_db = db.chain_state_db();

    if slot == 0 {
        // Nothing to do at slot 0 since that's the genesis block is hardcoded.
        return Ok(Vec::new());
    }

    let prev_slot = slot - 1;

    // TODO actually fetch the canonical block
    let blkids = l2blkman.get_blocks_at_height_blocking(prev_slot)?;
    if blkids.is_empty() {
        return Err(Error::NotReady.into());
    }

    let blkid = blkids[0];

    // Load the previous chain state.
    //
    // I'm not really sure why we're doing this here.  Maybe as a sanity check?
    let Some(chs) = chs_db.get_toplevel_state(prev_slot)? else {
        return Err(Error::MissingChainstate(blkid));
    };

    let chain_tip_block = chs.chain_tip_block();
    assert_eq!(chain_tip_block, blkid);

    // Assemble the context for the duty.
    let sign_block_context = SignBlockContext {
        slot,
        parent: blkid,
    };

    Ok(vec![SlotDuty::SignBlock(sign_block_context)])
}

// TODO change to EpochDuty
pub fn get_duties_for_epoch(
    epoch: u64,
    ident: Identity,
    l2blkman: Arc<L2BlockManager>,
    db: &impl Database,
) -> anyhow::Result<Vec<SlotDuty>> {
    error!("epoch duty extraction not implemented yet");
    let epoch_final_block = L2BlockCommitment::new(0, L2BlockId::default());

    Ok(Vec::new())
}
