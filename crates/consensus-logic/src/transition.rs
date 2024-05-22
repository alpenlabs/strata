//! Core state transition function.

use alpen_vertex_state::consensus::*;
use alpen_vertex_state::operation::*;
use alpen_vertex_state::sync_event::SyncEvent;

/// Processes the event given the current consensus state, producing some
/// output.
pub fn process_event(state: &ConsensusState, ev: &SyncEvent) -> ConsensusOutput {
    let mut writes = Vec::new();
    let mut actions = Vec::new();

    match ev {
        SyncEvent::L1BlockPosted(blkid) => {
            // TODO load it up and figure out what's there, see if we have to
            // load diffs from L1 or something
        }
        SyncEvent::L2BlockRecv(blkid) => {
            actions.push(SyncAction::TryCheckBlock(*blkid));
        }
        SyncEvent::L2BlockExec(blkid, ok) => {
            if *ok {
                // TODO also mark this as the new
                writes.push(ConsensusWrite::QueueL2Block(*blkid));
                actions.push(SyncAction::ExtendTip(*blkid));
            } else {
                actions.push(SyncAction::MarkInvalid(*blkid));
            }
        }
    }

    ConsensusOutput::new(writes, actions)
}
