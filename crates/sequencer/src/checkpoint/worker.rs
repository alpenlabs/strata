//! worker to monitor chainstate and create checkpoint entries.

use std::sync::Arc;

use strata_consensus_logic::csm::message::ClientUpdateNotif;
use strata_db::{traits::Database, types::CheckpointEntry};
use strata_primitives::{buf::Buf32, params::Params};
use strata_state::{
    batch::{BatchInfo, BootstrapState},
    client_state::ClientState,
};
use strata_storage::NodeStorage;
use strata_tasks::ShutdownGuard;
use thiserror::Error;
use tokio::sync::broadcast;
use tracing::*;

use super::CheckpointHandle;

#[derive(Debug, Error)]
enum Error {
    #[error("chain is not active yet")]
    ChainInactive,

    #[error("missing expected chainstate for blockidx {0}")]
    MissingIdxChainstate(u64),

    #[error("db: {0}")]
    Db(#[from] strata_db::errors::DbError),
}

/// Worker to monitor client state updates and create checkpoint entries
/// pending proof when previous proven checkpoint is finalized.
pub fn checkpoint_worker<D: Database>(
    shutdown: ShutdownGuard,
    mut cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    params: Arc<Params>,
    _database: Arc<D>,
    storage: Arc<NodeStorage>,
    checkpoint_handle: Arc<CheckpointHandle>,
) -> anyhow::Result<()> {
    let rollup_params_commitment = params.rollup().compute_hash();

    loop {
        if shutdown.should_shutdown() {
            warn!("received shutdown signal");
            break;
        }
        let update = match cupdate_rx.blocking_recv() {
            Ok(u) => u,
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!(%skipped, "overloaded, skipping dispatching some duties");
                continue;
            }
        };

        let state = update.new_state();

        let next_checkpoint_idx = get_next_batch_idx(state);
        // check if entry is already present
        if checkpoint_handle
            .get_checkpoint_blocking(next_checkpoint_idx)?
            .is_some()
        {
            continue;
        }

        let (batch_info, bootstrap_state) =
            match get_next_batch(state, storage.as_ref(), rollup_params_commitment) {
                Err(error) => {
                    warn!(?error, "Failed to get next batch");
                    continue;
                }
                Ok((b, bs)) => (b, bs),
            };

        let checkpoint_idx = batch_info.idx();
        // sanity check
        assert!(checkpoint_idx == next_checkpoint_idx);

        // else save a pending proof checkpoint entry
        debug!("save checkpoint pending proof: {}", checkpoint_idx);
        let entry = CheckpointEntry::new_pending_proof(batch_info, bootstrap_state);
        if let Err(e) = checkpoint_handle.put_checkpoint_and_notify_blocking(checkpoint_idx, entry)
        {
            warn!(?e, "Failed to save checkpoint at idx: {}", checkpoint_idx);
        }
    }
    Ok(())
}

fn get_next_batch_idx(state: &ClientState) -> u64 {
    match state.l1_view().last_finalized_checkpoint() {
        None => 0,
        Some(prev_checkpoint) => prev_checkpoint.batch_info.idx + 1,
    }
}

fn get_next_batch(
    state: &ClientState,
    storage: &NodeStorage,
    rollup_params_commitment: Buf32,
) -> Result<(BatchInfo, BootstrapState), Error> {
    let chsman = storage.chainstate();

    if !state.is_chain_active() {
        // before genesis
        debug!("chain not active, no duties created");
        return Err(Error::ChainInactive);
    };

    let Some(sync_state) = state.sync() else {
        return Err(Error::ChainInactive);
    };

    let tip_height = sync_state.chain_tip_height();
    let tip_id = *sync_state.chain_tip_blkid();

    // Still not ready to make a batch?  This should be only if the epoch is
    // something else.
    if tip_height == 0 {
        return Err(Error::ChainInactive);
    }

    match state.l1_view().last_finalized_checkpoint() {
        // Cool, we are producing first batch!
        None => {
            debug!(
                ?tip_height,
                ?tip_id,
                "No finalized checkpoint, creating new checkpiont"
            );

            let first_checkpoint_idx = 0;

            let current_l1_state = state
                .l1_view()
                .tip_verification_state()
                .ok_or(Error::ChainInactive)?;

            // Include blocks after genesis l1 height to last verified height
            let l1_range = (
                state.genesis_l1_height() + 1,
                current_l1_state.last_verified_block_num as u64,
            );
            let genesis_l1_state_hash = state
                .genesis_verification_hash()
                .ok_or(Error::ChainInactive)?;
            let current_l1_state_hash = current_l1_state.compute_hash().unwrap();
            let l1_transition = (genesis_l1_state_hash, current_l1_state_hash);

            // Start from first non-genesis l2 block height
            let l2_range = (1, tip_height);

            let initial_chain_state = chsman
                .get_toplevel_chainstate_blocking(0)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let initial_chain_state_root = initial_chain_state.compute_state_root();

            let current_chain_state = chsman
                .get_toplevel_chainstate_blocking(tip_height)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (initial_chain_state_root, current_chain_state_root);

            let new_batch = BatchInfo::new(
                first_checkpoint_idx,
                l1_range,
                l2_range,
                l1_transition,
                l2_transition,
                tip_id,
                (0, current_l1_state.total_accumulated_pow),
                rollup_params_commitment,
            );

            let genesis_bootstrap = new_batch.get_initial_bootstrap_state();
            Ok((new_batch, genesis_bootstrap))
        }
        Some(prev_checkpoint) => {
            let checkpoint = prev_checkpoint.batch_info.clone();

            let current_l1_state = state
                .l1_view()
                .tip_verification_state()
                .ok_or(Error::ChainInactive)?;
            let l1_range = (
                checkpoint.l1_range.1 + 1,
                current_l1_state.last_verified_block_num as u64,
            );
            let current_l1_state_hash = current_l1_state.compute_hash().unwrap();
            let l1_transition = (checkpoint.l1_transition.1, current_l1_state_hash);

            // Also, rather than tip heights, we might need to limit the max range a prover will
            // be proving
            let l2_range = (checkpoint.l2_range.1 + 1, tip_height);
            let current_chain_state = chsman
                .get_toplevel_chainstate_blocking(tip_height)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (checkpoint.l2_transition.1, current_chain_state_root);

            let new_batch = BatchInfo::new(
                checkpoint.idx + 1,
                l1_range,
                l2_range,
                l1_transition,
                l2_transition,
                tip_id,
                (
                    checkpoint.l1_pow_transition.1,
                    current_l1_state.total_accumulated_pow,
                ),
                rollup_params_commitment,
            );

            // If prev checkpoint was proved, use the bootstrap state of the prev checkpoint
            // else create a bootstrap state based on initial info of this batch
            let bootstrap_state = if prev_checkpoint.is_proved {
                prev_checkpoint.bootstrap_state.clone()
            } else {
                new_batch.get_initial_bootstrap_state()
            };
            Ok((new_batch, bootstrap_state))
        }
    }
}
