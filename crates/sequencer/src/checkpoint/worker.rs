//! worker to monitor chainstate and create checkpoint entries.

use std::sync::Arc;

use strata_consensus_logic::csm::message::ClientUpdateNotif;
use strata_db::{traits::Database, types::CheckpointEntry, DbError};
use strata_primitives::{buf::Buf32, l1::L1BlockCommitment, l2::L2BlockCommitment, params::Params};
use strata_state::{
    batch::{BaseStateCommitment, BatchInfo, BatchTransition},
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

        let (batch_info, batch_transition, base_state_commitment) =
            match get_next_batch(state, storage.as_ref(), rollup_params_commitment) {
                Err(error) => {
                    warn!(?error, "Failed to get next batch");
                    continue;
                }
                Ok((b, bt, bs)) => (b, bt, bs),
            };

        let checkpoint_idx = batch_info.epoch();
        // sanity check
        assert!(checkpoint_idx == next_checkpoint_idx);

        // else save a pending proof checkpoint entry
        debug!("save checkpoint pending proof: {}", checkpoint_idx);
        let entry =
            CheckpointEntry::new_pending_proof(batch_info, batch_transition, base_state_commitment);
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
        Some(prev_checkpoint) => prev_checkpoint.batch_info.epoch + 1,
    }
}

fn get_next_batch(
    state: &ClientState,
    storage: &NodeStorage,
    rollup_params_commitment: Buf32,
) -> Result<(BatchInfo, BatchTransition, BaseStateCommitment), Error> {
    if !state.is_chain_active() {
        debug!("chain not active, no duties created");
        return Err(Error::ChainInactive);
    }

    let sync_state = state.sync().ok_or(Error::ChainInactive)?;
    let tip_height = sync_state.chain_tip_height();
    let tip_id = *sync_state.chain_tip_blkid();

    if tip_height == 0 {
        return Err(Error::ChainInactive);
    }

    let chsman = storage.chainstate();

    // Fetch the current L1 verification state (required in both branches).
    let current_l1_state = state
        .l1_view()
        .tip_verification_state()
        .ok_or(Error::ChainInactive)?;
    let current_l1_state_hash = current_l1_state.compute_hash().unwrap();

    // Helper closures to get L1 and L2 block commitments.
    let get_l1_commitment = |height: u64| -> Result<L1BlockCommitment, Error> {
        let manifest = storage
            .l1()
            .get_block_manifest(height)?
            .ok_or(DbError::MissingL1BlockBody(height))?;
        Ok(L1BlockCommitment::new(height, manifest.block_hash()))
    };

    let get_l2_commitment = |height: u64| -> Result<L2BlockCommitment, Error> {
        let blocks = storage.l2().get_blocks_at_height_blocking(height)?;
        let block_id = blocks.first().ok_or(DbError::MissingL2State(height))?;
        Ok(L2BlockCommitment::new(height, *block_id))
    };

    match state.l1_view().last_finalized_checkpoint() {
        // --- Branch: First batch (no finalized checkpoint exists yet) ---
        None => {
            debug!(
                ?tip_height,
                ?tip_id,
                "No finalized checkpoint, creating new checkpoint"
            );
            let first_checkpoint_idx = 0;

            let genesis_l1_state_hash = state
                .genesis_verification_hash()
                .ok_or(Error::ChainInactive)?;

            // Determine the L1 range.
            let initial_l1_height = state.genesis_l1_height() + 1;
            let initial_l1_commitment = get_l1_commitment(initial_l1_height)?;
            let final_l1_height = current_l1_state.last_verified_block.height();
            let final_l1_commitment = get_l1_commitment(final_l1_height)?;
            let l1_range = (initial_l1_commitment, final_l1_commitment);
            let l1_transition = (genesis_l1_state_hash, current_l1_state_hash);

            // Determine the L2 range.
            let initial_l2_height = 1;
            let initial_l2_commitment = get_l2_commitment(initial_l2_height)?;
            let final_l2_commitment = L2BlockCommitment::new(tip_height, tip_id);
            let l2_range = (initial_l2_commitment, final_l2_commitment);

            // Compute the L2 chainstate transition.
            let initial_chain_state = chsman
                .get_toplevel_chainstate_blocking(0)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let initial_chain_state_root = initial_chain_state.compute_state_root();
            let current_chain_state = chsman
                .get_toplevel_chainstate_blocking(tip_height)?
                .ok_or(Error::MissingIdxChainstate(tip_height))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (initial_chain_state_root, current_chain_state_root);

            // Build the batch transition and batch info.
            let new_transition =
                BatchTransition::new(l1_transition, l2_transition, rollup_params_commitment);
            let new_batch = BatchInfo::new(first_checkpoint_idx, l1_range, l2_range);
            let genesis_state = new_transition.get_initial_base_state_commitment();

            Ok((new_batch, new_transition, genesis_state))
        }

        // --- Branch: Subsequent batches (using the last finalized checkpoint) ---
        Some(prev_checkpoint) => {
            let batch_info = prev_checkpoint.batch_info.clone();
            let batch_transition = prev_checkpoint.batch_transition.clone();

            // Build the L1 range for the new batch.
            let initial_l1_height = batch_info.l1_range.1.height() + 1;
            let initial_l1_commitment = get_l1_commitment(initial_l1_height)?;

            // Use the block id from the current verification state.
            let final_l1_commitment = current_l1_state.last_verified_block;
            let l1_range = (initial_l1_commitment, final_l1_commitment);
            let l1_transition = (batch_transition.l1_transition.1, current_l1_state_hash);

            // Build the L2 range for the new batch.
            let initial_l2_height = batch_info.l2_range.1.slot() + 1;
            let initial_l2_commitment = get_l2_commitment(initial_l2_height)?;
            let final_l2_commitment = L2BlockCommitment::new(tip_height, tip_id);
            let l2_range = (initial_l2_commitment, final_l2_commitment);
            let current_chain_state = chsman
                .get_toplevel_chainstate_blocking(tip_height)?
                .ok_or(Error::MissingIdxChainstate(tip_height))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (batch_transition.l2_transition.1, current_chain_state_root);

            let new_batch_info = BatchInfo::new(batch_info.epoch + 1, l1_range, l2_range);
            let new_transition =
                BatchTransition::new(l1_transition, l2_transition, rollup_params_commitment);

            let base_state_commitment = if prev_checkpoint.is_proved {
                prev_checkpoint.base_state_commitment.clone()
            } else {
                new_transition.get_initial_base_state_commitment()
            };

            Ok((new_batch_info, new_transition, base_state_commitment))
        }
    }
}
