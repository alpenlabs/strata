//! Consensus logic worker task.

use std::sync::mpsc;

use tracing::*;

use alpen_vertex_db::{errors::DbResult, traits::Database};
use alpen_vertex_state::{
    block::L2BlockId,
    consensus::ConsensusState,
    operation::{ConsensusOutput, ConsensusWrite, SyncAction},
    sync_event::SyncEvent,
};

use crate::{errors::Error, message::Message, transition};

/// Mutatble worker state that we modify in the consensus worker task.
///
/// Not meant to be shared across threads.
pub struct WorkerState<D: Database> {
    /// Underlying database hierarchy that writes ultimately end up on.
    database: D,

    /// Last event idx we've processed.
    last_processed_event: u64,

    /// Current state idx, corresponding to events.
    cur_state_idx: u64,

    /// Current consensus state we use when performing updates.
    cur_consensus_state: ConsensusState,

    /// Current hard chain tip.
    cur_chain_tip: L2BlockId,
}

impl<D: Database> WorkerState<D> {
    fn get_sync_event(&self, ev_idx: u64) -> DbResult<Option<SyncEvent>> {
        // TODO add an accessor to the database type to get the syncevent
        // provider and then call that
        unimplemented!()
    }

    /// Tries to apply the consensus output to the current state, storing things
    /// in the database.
    fn apply_consensus_writes(&mut self, outp: Vec<ConsensusWrite>) -> Result<(), Error> {
        // TODO
        Ok(())
    }

    /// Extends the chain tip by a block.  The referenced block must have the
    /// current chain tip as its parent.
    fn extend_tip(&mut self, blkid: L2BlockId) -> Result<(), Error> {
        // TODO
        Ok(())
    }

    /// Rolls up back to the specified block.
    fn rollback_to_block(&mut self, blkid: L2BlockId) -> Result<(), Error> {
        // TODO
        Ok(())
    }
}

/// Receives messages from channel to update consensus state with.
fn consensus_worker_task<D: Database>(
    mut state: WorkerState<D>,
    inp_msg_ch: mpsc::Receiver<Message>,
) -> Result<(), Error> {
    while let Some(msg) = inp_msg_ch.recv().ok() {
        if let Err(e) = process_msg(&mut state, &msg) {
            error!(err = %e, "failed to process sync message");
        }
    }

    info!("consensus task exiting");

    Ok(())
}

fn process_msg<D: Database>(state: &mut WorkerState<D>, msg: &Message) -> Result<(), Error> {
    match msg {
        Message::EventInput(idx) => {
            let ev = state
                .get_sync_event(*idx)?
                .ok_or(Error::MissingSyncEvent(*idx))?;

            handle_sync_event(state, *idx, &ev)?;
            Ok(())
        }
    }
}

fn handle_sync_event<D: Database>(
    state: &mut WorkerState<D>,
    idx: u64,
    event: &SyncEvent,
) -> Result<(), Error> {
    // Perform the main step of deciding what the output we're operating on.
    let outp = transition::process_event(&state.cur_consensus_state, event);
    let (writes, actions) = outp.into_parts();
    state.apply_consensus_writes(writes)?;

    for action in actions {
        match action {
            SyncAction::TryCheckBlock(blkid) => {
                // TODO trigger call to update engine controller
            }
            SyncAction::ExtendTip(blkid) => {
                state.extend_tip(blkid)?;
            }
            SyncAction::RevertTip(blkid) => {
                state.rollback_to_block(blkid)?;
            }
            SyncAction::MarkInvalid(blkid) => {
                // TODO not sure what this should entail yet
            }
        }
    }

    Ok(())
}
