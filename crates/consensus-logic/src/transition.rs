//! Core state transition function.

use alpen_vertex_db::errors::DbError;
use alpen_vertex_db::traits::Database;
use alpen_vertex_db::traits::L1DataProvider;
use alpen_vertex_state::consensus::*;
use alpen_vertex_state::operation::*;
use alpen_vertex_state::sync_event::SyncEvent;

/// Processes the event given the current consensus state, producing some
/// output.  This can return database errors.
pub fn process_event<D: Database>(
    state: &ConsensusState,
    ev: &SyncEvent,
    database: &D,
) -> Result<ConsensusOutput, DbError> {
    let mut writes = Vec::new();
    let mut actions = Vec::new();

    match ev {
        SyncEvent::L1Block(height, l1blkid) => {
            // FIXME this doesn't do any SPV checks to make sure we only go to
            // a longer chain, it just does it unconditionally
            let l1prov = database.l1_provider();
            let blkmf = l1prov.get_block_manifest(*height)?;

            // TODO do the consensus checks

            writes.push(ConsensusWrite::AcceptL1Block(*l1blkid));

            // TODO if we have some number of L1 blocks finalized, also emit an
            // `UpdateBuried` write
        }
        SyncEvent::L1DABatch(blkids) => {
            // TODO load it up and figure out what's there, see if we have to
            // load diffs from L1 or something
        }
        SyncEvent::NewTipBlock(blkid) => {
            // TODO better checks here
            writes.push(ConsensusWrite::AcceptL2Block(*blkid));
            actions.push(SyncAction::UpdateTip(*blkid));
        }
    }

    Ok(ConsensusOutput::new(writes, actions))
}
