//! Core state transition function.
#![allow(unused)] // still under development

use tracing::*;

use alpen_express_db::traits::{Database, L1DataProvider, L2DataProvider};
use alpen_express_primitives::prelude::*;
use alpen_express_state::client_state::*;
use alpen_express_state::operation::*;
use alpen_express_state::sync_event::SyncEvent;

use crate::errors::*;

/// Processes the event given the current consensus state, producing some
/// output.  This can return database errors.
pub fn process_event<D: Database>(
    state: &ClientState,
    ev: &SyncEvent,
    database: &D,
    params: &Params,
) -> Result<ClientUpdateOutput, Error> {
    let mut writes = Vec::new();
    let mut actions = Vec::new();

    match ev {
        SyncEvent::L1Block(height, l1blkid) => {
            // FIXME this doesn't do any SPV checks to make sure we only go to
            // a longer chain, it just does it unconditionally
            let l1prov = database.l1_provider();
            let _blkmf = l1prov.get_block_manifest(*height)?;

            // TODO do the consensus checks

            writes.push(ClientStateWrite::AcceptL1Block(*l1blkid));

            // TODO if we have some number of L1 blocks finalized, also emit an
            // `UpdateBuried` write
            if *height >= params.rollup().l1_reorg_safe_depth + state.buried_l1_height() {
                writes.push(ClientStateWrite::UpdateBuried(state.buried_l1_height() + 1));
            }

            let l1v = state.l1_view();

            if let Some(ss) = state.sync() {
                // TODO figure out what to do here
            } else {
                let horizon_ht = params.rollup.horizon_l1_height;
                let genesis_ht = params.rollup.genesis_l1_height;

                // TODO make params configurable
                let genesis_threshold = genesis_ht + 3;

                // If necessary, activeate the chain!
                if !state.is_chain_active() && *height >= genesis_threshold {
                    debug!("emitting chain activation");
                    writes.push(ClientStateWrite::ActivateChain);
                }
            }
        }

        SyncEvent::L1Revert(to_height) => {
            // TODO
            let l1prov = database.l1_provider();
            let blkmf = l1prov.get_block_manifest(*to_height)?.unwrap();
            let blkid = blkmf.block_hash().into();
            writes.push(ClientStateWrite::RollbackL1BlocksTo(blkid));
        }

        SyncEvent::L1DABatch(blkids) => {
            if blkids.is_empty() {
                warn!("empty L1DABatch");
            }

            if let Some(ss) = state.sync() {
                // TODO load it up and figure out what's there, see if we have to
                // load the state updates from L1 or something
                let l2prov = database.l2_provider();

                for id in blkids {
                    let _block = l2prov
                        .get_block_data(*id)?
                        .ok_or(Error::MissingL2Block(*id))?;

                    // TODO do whatever changes we have to to accept the new block
                }

                let last = blkids.last().unwrap();
                writes.push(ClientStateWrite::UpdateFinalized(*last));
                actions.push(SyncAction::FinalizeBlock(*last))
            } else {
                // TODO we can expand this later to make more sense
                return Err(Error::MissingClientSyncState);
            }
        }

        SyncEvent::ComputedGenesis(gblkid) => {
            // Just construct the sync state for the genesis.
            let ss = SyncState::from_genesis_blkid(*gblkid);
            writes.push(ClientStateWrite::ReplaceSync(Box::new(ss)));
        }

        SyncEvent::NewTipBlock(blkid) => {
            let l2prov = database.l2_provider();
            let _block = l2prov
                .get_block_data(*blkid)?
                .ok_or(Error::MissingL2Block(*blkid))?;

            // TODO better checks here
            writes.push(ClientStateWrite::AcceptL2Block(*blkid));
            actions.push(SyncAction::UpdateTip(*blkid));
        }
    }

    Ok(ClientUpdateOutput::new(writes, actions))
}
