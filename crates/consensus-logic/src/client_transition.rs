//! Core state transition function.

use alpen_vertex_db::sync_event::schemas::SyncEventWithTimestamp;
use alpen_vertex_db::traits::{Database, L1DataProvider, L2DataProvider};
use alpen_vertex_primitives::prelude::*;
use alpen_vertex_state::client_state::*;
use alpen_vertex_state::operation::*;
use alpen_vertex_state::sync_event::SyncEvent;

use crate::errors::*;

/// Processes the event given the current consensus state, producing some
/// output.  This can return database errors.
pub fn process_event<D: Database>(
    _state: &ClientState,
    ev: &SyncEvent,
    database: &D,
    _params: &Params,
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
        }

        SyncEvent::L1Revert(_to_height) => {
            // TODO
        }

        SyncEvent::L1DABatch(blkids) => {
            // TODO load it up and figure out what's there, see if we have to
            // load the state updates from L1 or something
            let l2prov = database.l2_provider();

            for id in blkids {
                let _block = l2prov
                    .get_block_data(*id)?
                    .ok_or(Error::MissingL2Block(*id))?;

                // TODO do whatever changes we have to to accept the new block
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
