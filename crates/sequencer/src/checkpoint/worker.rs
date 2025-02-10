//! worker to monitor chainstate and create checkpoint entries.

use std::sync::Arc;

use strata_consensus_logic::csm::message::ClientUpdateNotif;
use strata_db::{traits::Database, types::CheckpointEntry, DbError};
use strata_primitives::{
    buf::Buf32,
    epoch::EpochCommitment,
    l1::{L1BlockCommitment, L1BlockId, L1BlockManifest},
    l2::{L2BlockCommitment, L2BlockId},
    params::{Params, RollupParams},
};
use strata_state::{
    batch::{BaseStateCommitment, BatchInfo, BatchTransition, EpochSummary},
    block::L2BlockBundle,
    chain_state::Chainstate,
    client_state::ClientState,
    header::*,
    l1::HeaderVerificationState,
};
use strata_storage::{ChainstateManager, L1BlockManager, L2BlockManager, NodeStorage};
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

    #[error("missing checkpoint for epoch {0}")]
    MissingCheckpoint(u64),

    #[error("missing L1 block from database {0}")]
    MissingL1Block(L1BlockId),

    #[error("missing L2 block from database {0}")]
    MissingL2Block(L2BlockId),

    #[error("stored L1 block {0:?} scanned using wrong epoch (got {1}, exp {2})")]
    L1BlockWithWrongEpoch(L1BlockId, u64, u64),

    /// If we can't find the start block or something.
    #[error("malformed epoch {0:?}")]
    MalformedEpoch(EpochCommitment),

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

        let next_checkpoint_idx = get_next_checkpoint_idx(state);
        // check if entry is already present
        if checkpoint_handle
            .get_checkpoint_blocking(next_checkpoint_idx)?
            .is_some()
        {
            continue;
        }

        let (batch_info, batch_transition, base_state_commitment) =
            match get_next_batch(state, storage.as_ref(), rollup_params_commitment) {
                Err(e) => {
                    warn!(err = %e, "Failed to get next batch");
                    continue;
                }
                Ok(data) => data,
            };

        let checkpoint_idx = batch_info.epoch();

        // sanity check
        assert_eq!(checkpoint_idx, next_checkpoint_idx);

        // else save a pending proof checkpoint entry
        debug!(%checkpoint_idx, "saving checkpoint pending proof");
        let entry =
            CheckpointEntry::new_pending_proof(batch_info, batch_transition, base_state_commitment);
        if let Err(e) = checkpoint_handle.put_checkpoint_and_notify_blocking(checkpoint_idx, entry)
        {
            warn!(%checkpoint_idx, err = %e, "failed to save checkpoint");
        }
    }
    Ok(())
}

fn get_next_checkpoint_idx(state: &Chainstate) -> u64 {
    state.cur_epoch()
}

pub struct CheckpointPrepData {
    info: BatchInfo,
    tsn: BatchTransition,
    commitment: BaseStateCommitment,
}

impl CheckpointPrepData {
    pub fn new(info: BatchInfo, tsn: BatchTransition, commitment: BaseStateCommitment) -> Self {
        Self {
            info,
            tsn,
            commitment,
        }
    }
}

/*
fn get_next_batch(
    chainstate: &Chainstate,
    storage: &NodeStorage,
    rollup_params_commitment: Buf32,
) -> Result<CheckpointPrepData, Error> {
    let tip_slot = chainstate.chain_tip_slot();
    let tip_blkid = *chainstate.chain_tip_blkid();

    if tip_slot == 0 {
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

    match chainstate.cur_epoch() {
        // --- Branch: First batch (no finalized checkpoint exists yet) ---
        0 => {
            debug!(
                %tip_slot,
                %tip_blkid,
                "No finalized checkpoint, creating new checkpoint"
            );
            let first_checkpoint_idx = 0;

            let genesis_l1_state_hash = state
                .genesis_verification_hash()
                .ok_or(Error::ChainInactive)?;

            // Determine the L1 range.
            let initial_l1_height = state.genesis_l1_height() + 1;
            let initial_l1_commitment = get_l1_commitment(initial_l1_height)?;
            let final_l1_height = current_l1_state.last_verified_block_num as u64;
            let final_l1_commitment = get_l1_commitment(final_l1_height)?;
            let l1_range = (initial_l1_commitment, final_l1_commitment);
            let l1_transition = (genesis_l1_state_hash, current_l1_state_hash);

            // Determine the L2 range.
            let initial_l2_height = 1;
            let initial_l2_commitment = get_l2_commitment(initial_l2_height)?;
            let final_l2_commitment = L2BlockCommitment::new(tip_slot, tip_blkid);
            let l2_range = (initial_l2_commitment, final_l2_commitment);

            // Compute the L2 chainstate transition.
            let initial_chain_state = chsman
                .get_toplevel_chainstate_blocking(0)?
                .ok_or(Error::MissingIdxChainstate(0))?;
            let initial_chain_state_root = initial_chain_state.compute_state_root();
            let current_chain_state = chsman
                .get_toplevel_chainstate_blocking(tip_slot)?
                .ok_or(Error::MissingIdxChainstate(tip_slot))?;
            let current_chain_state_root = current_chain_state.compute_state_root();
            let l2_transition = (initial_chain_state_root, current_chain_state_root);

            // Build the batch transition and batch info.
            let new_transition =
                BatchTransition::new(l1_transition, l2_transition, rollup_params_commitment);
            let new_batch = BatchInfo::new(first_checkpoint_idx, l1_range, l2_range);
            let genesis_state = new_transition.get_initial_base_state_commitment();

            Ok(CheckpointPrepData::new(
                new_batch,
                new_transition,
                genesis_state,
            ))
        }

        // --- Branch: Subsequent batches (using the last finalized checkpoint) ---
        epoch => {
            let prev_checkpoint = storage
                .checkpoint()
                .get_checkpoint_blocking(epoch)?
                .ok_or(Error::MissingCheckpoint(epoch))?;

            let batch_info = prev_checkpoint.batch_info.clone();
            let batch_transition = prev_checkpoint.batch_transition.clone();

            // Build the L1 range for the new batch.
            let initial_l1_height = batch_info.l1_range.1.height() + 1;
            let initial_l1_commitment = get_l1_commitment(initial_l1_height)?;
            let final_l1_height = current_l1_state.last_verified_block_num as u64;
            // Use the block id from the current verification state.
            let final_l1_commitment =
                L1BlockCommitment::new(final_l1_height, current_l1_state.last_verified_block_hash);
            let l1_range = (initial_l1_commitment, final_l1_commitment);
            let l1_transition = (batch_transition.l1_transition.1, current_l1_state_hash);

            // Build the L2 range for the new batch.
            let initial_l2_height = batch_info.l2_range.1.slot() + 1;
            let initial_l2_commitment = get_l2_commitment(initial_l2_height)?;
            let final_l2_commitment = L2BlockCommitment::new(tip_slot, tip_blkid);
            let l2_range = (initial_l2_commitment, final_l2_commitment);
            let current_chain_state = chsman
                .get_toplevel_chainstate_blocking(tip_slot)?
                .ok_or(Error::MissingIdxChainstate(tip_slot))?;
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

            Ok(CheckpointPrepData::new(
                new_batch_info,
                new_transition,
                base_state_commitment,
            ))
        }
    }
}
*/

/// Creates the CPD for a completed epoch from an epoch summary, if possible.
fn create_checkpoint_prep_data_from_summary(
    summary: &EpochSummary,
    storage: &NodeStorage,
    params: &RollupParams,
) -> Result<CheckpointPrepData, Error> {
    let l1man = storage.l1();
    let l2man = storage.l2();
    let rollup_params_hash = params.compute_hash();

    let prev_epoch = summary.epoch() - 1;
    let prev_checkpoint = storage
        .checkpoint()
        .get_checkpoint_blocking(prev_epoch)?
        .ok_or(Error::MissingCheckpoint(prev_epoch))?;

    // There's some special handling we have to do if we're the genesis epoch.
    let is_genesis_epoch = summary.epoch() == 0;
    let prev_summary = if is_genesis_epoch {
        let ps = storage
            .checkpoint()
            .get_epoch_summary_blocking(summary.get_prev_epoch_commitment().unwrap())?
            .ok_or(Error::MissingCheckpoint(summary.epoch() - 1))?;
        Some(ps)
    } else {
        None
    };

    // Determine the ranges for each of the fields we commit to.
    let l1_start_height = if let Some(ps) = prev_summary {
        ps.new_l1().height() + 1
    } else {
        params.genesis_l1_height + 1
    };

    // Reconstruct the L1 range.
    let l1_start_mf = fetch_l1_block_manifest(l1_start_height, l1man)?;
    let l1_start_block = L1BlockCommitment::new(l1_start_height, l1_start_mf.block_hash());
    let l1_range = (l1_start_block, *summary.new_l1());

    // Compute the new L1 sync state commitments.
    // FIXME this is wrong but it's hard to get this state properly now
    let tip_vs = HeaderVerificationState::default();
    let genesis_vs_hash = tip_vs.compute_hash().unwrap();
    let tip_vs_hash = tip_vs.compute_hash().unwrap();
    let l1_transition = (genesis_vs_hash, tip_vs_hash);

    // Now just pull out the data about the blocks from the transition here.
    //
    // There's a slight weirdness here.  The "range" refers to the first block
    // of the epoch, but the "transition" refers to the final state (ie last
    // block, for now) of the previous epoch.
    let l2_blocks = get_epoch_l2_headers(summary, l2man)?;
    let first_block = l2_blocks.first().unwrap();
    let initial_l2_commitment =
        L2BlockCommitment::new(first_block.blockidx(), first_block.get_blockid());
    let l2_range = (initial_l2_commitment, *summary.terminal());
    let l2_transition = (
        prev_summary.map(|ps| *ps.final_state()).unwrap_or_default(),
        *summary.final_state(),
    );

    // Assemble the final parts together.
    let new_transition = BatchTransition::new(l1_transition, l2_transition, rollup_params_hash);
    let new_batch_info = BatchInfo::new(summary.epoch(), l1_range, l2_range);

    // TODO make sure this is correct
    let base_state_commitment = if prev_checkpoint.is_proof_ready() {
        prev_checkpoint
            .into_batch_checkpoint()
            .base_state_commitment()
            .clone()
    } else {
        new_transition.get_initial_base_state_commitment()
    };

    Ok(CheckpointPrepData::new(
        new_batch_info,
        new_transition,
        base_state_commitment,
    ))
}

fn get_epoch_l2_headers(
    summary: &EpochSummary,
    l2man: &L2BlockManager,
) -> Result<Vec<L2BlockHeader>, Error> {
    let limit = 5000; // TODO make a const

    let mut headers = Vec::new();

    let terminal = fetch_l2_block(summary.terminal().blkid(), l2man)?;
    headers.push(terminal.header().header().clone());

    // The loop keeps fetching the current header's parent block and extending
    // the list with it.
    //
    // The break conditions are a little weird so we use a bare `loop`.
    loop {
        if headers.len() >= limit {
            return Err(Error::MalformedEpoch(summary.get_epoch_commitment()));
        }

        let cur = headers.last().unwrap();
        let cur_parent = cur.parent();

        // If we're at the genesis block we can just exit.
        if cur.blockidx() == 0 {
            break;
        }

        // If the current block's parent is the previous epoch's terminal block,
        // we can just break.
        if cur_parent == summary.prev_terminal().blkid() {
            break;
        }

        // Otherwise, just fetch the block and attach it.
        let block = fetch_l2_block(cur_parent, l2man)?;
        headers.push(block.header().header().clone());
    }

    // Also reverse the headers list so that earlier blocks are at the beginning.
    headers.reverse();

    Ok(headers)
}

/// Gets L1 epoch manifests back to a previous height.  This height should be
/// the last L1 block we want to include.  This would be one higher than the
/// previous epoch's new L1 block, or the genesis trigger height.
///
/// # Panics
///
/// If the prev epochs's L1 block is after the current summary's L1 block.
fn get_epoch_l1_manifests(
    summary: &EpochSummary,
    initial_l1_height: u64,
    l1man: &L1BlockManager,
) -> Result<Vec<L1BlockManifest>, Error> {
    if initial_l1_height > summary.new_l1().height() {
        panic!("ckptworker: invalid L1 blocks query");
    }

    let start_height = summary.new_l1().height();
    let break_height = initial_l1_height;
    let limit = 2016; // TODO make a const?
    let prev_epoch = summary.epoch() - 1;

    let mut manifests = Vec::new();

    // This isn't actually necessary due to how the loop works, but it will be
    // necessary when we start fetching by blkid and we need to have an initial
    // block to get the parent of.
    let terminal = fetch_block_manifest_at_epoch(start_height, prev_epoch, l1man)?;
    manifests.push(terminal);

    // This keeps fetches the blocks in reverse since we want to switch this to
    // using blkids for db queries.  This should be using the parent blkids
    // instead.
    //
    // We also ensure that the manifest was generated using the previous epoch's
    // scan configuration.
    loop {
        if manifests.len() >= limit {
            return Err(Error::MalformedEpoch(summary.get_epoch_commitment()));
        }

        // Kinda hacky math but it works.
        let cur_height = start_height - manifests.len() as u64;

        let mf = fetch_block_manifest_at_epoch(cur_height, prev_epoch, l1man)?;
        manifests.push(mf);

        // If the next height is the final block we wanted, then we're done.
        if cur_height == break_height {
            break;
        }
    }

    // Similarly to before, also reverse it so it's in order.
    manifests.reverse();

    Ok(manifests)
}

fn fetch_l2_block(blkid: &L2BlockId, l2man: &L2BlockManager) -> Result<L2BlockBundle, Error> {
    Ok(l2man
        .get_block_data_blocking(blkid)?
        .ok_or(Error::MissingL2Block(*blkid))?)
}

fn fetch_chainstate(slot: u64, chsman: &ChainstateManager) -> Result<Chainstate, Error> {
    chsman
        .get_toplevel_chainstate_blocking(slot)?
        .ok_or(Error::MissingIdxChainstate(slot))
}

fn fetch_l1_block_manifest(height: u64, l1man: &L1BlockManager) -> Result<L1BlockManifest, Error> {
    Ok(l1man
        .get_block_manifest(height)?
        .ok_or(DbError::MissingL1BlockBody(height))?)
}

/// Fetches and L1 block manifest, checking that the manifest that we found
/// reported as specific epoch index.
// TODO maybe convert this fn to use epoch commitments?
fn fetch_block_manifest_at_epoch(
    height: u64,
    epoch: u64,
    l1man: &L1BlockManager,
) -> Result<L1BlockManifest, Error> {
    let mf = fetch_l1_block_manifest(height, l1man)?;

    if mf.epoch() != epoch {
        return Err(Error::L1BlockWithWrongEpoch(
            mf.block_hash(),
            mf.epoch(),
            epoch,
        ));
    }

    Ok(mf)
}
