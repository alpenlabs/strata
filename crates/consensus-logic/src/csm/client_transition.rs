//! Core state transition function.
#![allow(unused)] // still under development

use std::cmp::min;

use bitcoin::block::Header;
use strata_db::traits::{ChainstateDatabase, Database, L1Database, L2BlockDatabase};
use strata_primitives::{
    batch::{BatchInfo, Checkpoint, L1CommittedCheckpoint},
    l1::{get_btc_params, HeaderVerificationState, L1BlockCommitment, L1BlockId},
    prelude::*,
};
use strata_state::{
    block::{self, L2BlockBundle},
    chain_state::Chainstate,
    client_state::*,
    header::L2Header,
    id::L2BlockId,
    operation::*,
    sync_event::SyncEvent,
};
use strata_storage::NodeStorage;
use tracing::*;
use zkaleido::ProofReceipt;

use crate::{checkpoint_verification::verify_proof, errors::*, genesis::make_genesis_block};

/// Interface for external context necessary specifically for event validation.
pub trait EventContext {
    fn get_l1_block_manifest(&self, height: u64) -> Result<L1BlockManifest, Error>;
    fn get_l2_block_data(&self, blkid: &L2BlockId) -> Result<L2BlockBundle, Error>;
    fn get_toplevel_chainstate(&self, slot: u64) -> Result<Chainstate, Error>;
}

/// Event context using the main node storage interfaace.
pub struct StorageEventContext<'c> {
    storage: &'c NodeStorage,
}

impl<'c> StorageEventContext<'c> {
    pub fn new(storage: &'c NodeStorage) -> Self {
        Self { storage }
    }
}

impl EventContext for StorageEventContext<'_> {
    fn get_l1_block_manifest(&self, height: u64) -> Result<L1BlockManifest, Error> {
        self.storage
            .l1()
            .get_block_manifest(height)?
            .ok_or(Error::MissingL1BlockHeight(height))
    }

    fn get_l2_block_data(&self, blkid: &L2BlockId) -> Result<L2BlockBundle, Error> {
        self.storage
            .l2()
            .get_block_data_blocking(blkid)?
            .ok_or(Error::MissingL2Block(*blkid))
    }

    fn get_toplevel_chainstate(&self, slot: u64) -> Result<Chainstate, Error> {
        self.storage
            .chainstate()
            .get_toplevel_chainstate_blocking(slot)?
            .ok_or(Error::MissingIdxChainstate(slot))
    }
}

/// Processes the event given the current consensus state, producing some
/// output.  This can return database errors.
pub fn process_event(
    state: &mut ClientStateMut,
    ev: &SyncEvent,
    context: &impl EventContext,
    params: &Params,
) -> Result<(), Error> {
    match ev {
        SyncEvent::L1Block(block) => {
            let height = block.height();

            // If the block is before genesis we don't care about it.
            // TODO maybe put back pre-genesis tracking?
            if height < params.rollup().genesis_l1_height {
                #[cfg(test)]
                eprintln!("early L1 block at h={height}, you may have set up the test env wrong");

                warn!(%height, "ignoring unexpected L1Block event before horizon");
                return Ok(());
            }

            // This doesn't do any SPV checks to make sure we only go to a
            // a longer chain, it just does it unconditionally.  This is fine,
            // since we'll be refactoring this more deeply soonish.
            let block_mf = context.get_l1_block_manifest(height)?;
            handle_block(state, block, &block_mf, context, params)?;
            Ok(())
        }

        SyncEvent::L1Revert(block) => {
            /*let buried = state.state().l1_view().buried_l1_height();
            if *to_height < buried {
                error!(%to_height, %buried, "got L1 revert below buried height");
                return Err(Error::ReorgTooDeep(*to_height, buried));
            }*/

            // TODO move this logic out into this function
            state.rollback_l1_blocks(*block);
            Ok(())
        }
    }
}

/*
        SyncEvent::L1BlockGenesis(block, l1_verification_state) => {
            let height = block.height();
            debug!(%height, "received L1BlockGenesis");

            let horizon_ht = params.rollup().horizon_l1_height;
            let genesis_ht = params.rollup().genesis_l1_height;

            let state_ht = l1_verification_state.last_verified_block_num as u64;
            if genesis_ht != state_ht {
                // FIXME bad error form
                let error_msg = format!(
                    "Expected height: {} Found height: {} in state",
                    genesis_ht, state_ht
                );
                return Err(Error::GenesisFailed(error_msg));
            }

            let threshold = params.rollup.l1_reorg_safe_depth;
            let genesis_threshold = genesis_ht + threshold as u64;

            let active = state.state().is_chain_active();
            debug!(%genesis_threshold, %genesis_ht, %active, "Inside activate chain");

            // Construct the block state for the genesis trigger block and insert it.
            let genesis_mf = context.get_l1_block_manifest(genesis_ht)?;
            let genesis_istate = process_genesis_trigger_block(&genesis_mf, params.rollup())?;
            state.accept_l1_block_state(*block, genesis_istate);

            // If necessary, activate the chain!
            if !active && height >= genesis_threshold {
                debug!("emitting chain activation");
                let genesis_block = make_genesis_block(params);

                state.activate_chain();
                state.update_verification_state(l1_verification_state.clone());
                state.set_sync_state(SyncState::from_genesis_blkid(
                    genesis_block.header().get_blockid(),
                ));

                state.push_action(SyncAction::L2Genesis(
                    l1_verification_state.last_verified_block_hash,
                ));
            }

            Ok(())
        }
*/

fn handle_block(
    state: &mut ClientStateMut,
    block: &L1BlockCommitment,
    block_mf: &L1BlockManifest,
    context: &impl EventContext,
    params: &Params,
) -> Result<(), Error> {
    let height = block.height();
    let l1blkid = block.blkid();

    /*let cur_seen_tip_height = state
    .state()
    .get_tip_l1_block()
    .map(|block| block.height())
    .unwrap_or(params.rollup().genesis_l1_height - 1);*/

    // Do the consensus checks
    /*if let Some(l1_vs) = l1_vs {
        let l1_vs_height = l1_vs.last_verified_block_num as u64;
        let mut updated_l1vs = l1_vs.clone();
        for height in (l1_vs_height + 1..cur_seen_tip_height) {
            let block_mf = context.get_l1_block_manifest(height)?;
            let header: Header =
                bitcoin::consensus::deserialize(block_mf.header()).unwrap();
            updated_l1vs =
                updated_l1vs.check_and_update_continuity_new(&header, &get_btc_params());
        }
        state.update_verification_state(updated_l1vs);
    }*/

    // Only accept the block if it's the next block in the chain we expect to accept.
    /*if next_exp_height > params.rollup().horizon_l1_height {
                // TODO check that the new block we're trying to add has the same parent as the tip
                // block
                let cur_tip_block = context.get_l1_block_manifest(cur_seen_tip_height)?;
    }*/

    let next_exp_height = state.state().next_exp_l1_block();

    let old_final_epoch = state.state().get_declared_final_epoch();

    // We probably should have gotten the L1Genesis message by now but
    // let's just do this anyways.
    if height == params.rollup().genesis_l1_height {
        // Do genesis here.
        let istate = process_genesis_trigger_block(&block_mf, params.rollup())?;
        state.accept_l1_block_state(block, istate);
        state.activate_chain();

        // Also have to set this.
        let genesis_block = make_genesis_block(params);
        state.set_sync_state(SyncState::from_genesis_blkid(
            genesis_block.header().get_blockid(),
        ));

        state.push_action(SyncAction::L2Genesis(*block.blkid()));
    } else if height == next_exp_height {
        // Do normal L1 block extension here.
        let prev_istate = state
            .state()
            .get_internal_state(height - 1)
            .expect("clientstate: missing expected block state");

        let new_istate = process_l1_block(prev_istate, height, &block_mf, params.rollup())?;
        state.accept_l1_block_state(block, new_istate);

        // TODO make max states configurable
        let max_states = 20;
        let total_states = state.state().internal_state_cnt();
        if total_states > max_states {
            let excess = total_states - max_states;
            let base_block = state
                .state()
                .get_deepest_l1_block()
                .expect("clienttsn: missing oldest state");
            state.discard_old_l1_states(base_block.height() + excess as u64);
        }
    } else {
        // If it's below the expected height then it's possible it's
        // just a tracking inconsistentcy, let's make sure we don't
        // already have it.
        if height < next_exp_height {
            if let Some(istate) = state.state().get_internal_state(height) {
                let internal_blkid = istate.blkid();
                if internal_blkid == l1blkid {
                    warn!(%next_exp_height, %height, "ignoring possible duplicate in-chain block");
                } else {
                    error!(%next_exp_height, %height, %internal_blkid, "given competing L1 block without reorg event, possible chain tracking issue");
                    return Err(Error::CompetingBlock(height, *internal_blkid, *l1blkid));
                }
            }
        }

        #[cfg(test)]
        eprintln!("not sure what to do here h={height} exp={next_exp_height}");
        return Err(Error::OutOfOrderL1Block(next_exp_height, height, *l1blkid));
    }

    // If there's a new epoch finalized that's better than the old one, update
    // the declared one.
    let new_final_epoch = state.state().get_apparent_finalized_epoch();
    match (old_final_epoch, new_final_epoch) {
        (None, Some(new)) => {
            state.set_decl_final_epoch(new);
        }
        (Some(old), Some(new)) if new.epoch() > old.epoch() => {
            state.set_decl_final_epoch(new);
        }
        _ => {}
    }

    // Emit the action to submit the finalized block.
    if let Some(decl_epoch) = state.state().get_declared_final_epoch() {
        state.push_action(SyncAction::FinalizeEpoch(*decl_epoch));
    }

    // If we have some number of L1 blocks finalized, also emit an `UpdateBuried` write.
    let safe_depth = params.rollup().l1_reorg_safe_depth as u64;
    let maturable_height = next_exp_height.saturating_sub(safe_depth);

    if maturable_height > params.rollup().horizon_l1_height && state.state().is_chain_active() {
        handle_mature_l1_height(state, maturable_height, context);
    }

    Ok(())
}

// TODO figure out what to do with this code
/*
      SyncEvent::L1DABatch(height, checkpoints) => {
          debug!(%height, "received L1DABatch");

          if let Some(ss) = state.state().sync() {
              let proof_verified_checkpoints =
                  filter_verified_checkpoints(state.state(), checkpoints, params.rollup());

              // When DABatch appears, it is only confirmed at the moment. These will be finalized
              // only when the corresponding L1 block is buried enough
              if !proof_verified_checkpoints.is_empty() {
                  // Copy out all the basic checkpoint data into dedicated
                  // structures for it.
                  let ckpts = proof_verified_checkpoints
                      .iter()
                      .map(|batch_checkpoint_with_commitment| {
                          let batch_checkpoint =
                              &batch_checkpoint_with_commitment.batch_checkpoint;
                          L1Checkpoint::new(
                              batch_checkpoint.batch_info().clone(),
                              batch_checkpoint.batch_transition().clone(),
                              batch_checkpoint.base_state_commitment().clone(),
                              !batch_checkpoint.proof().is_empty(),
                              *height,
                          )
                      })
                      .collect::<Vec<_>>();
                  state.accept_checkpoints(&ckpts);

                  state.push_action(SyncAction::WriteCheckpoints(
                      *height,
                      proof_verified_checkpoints,
                  ));
              }
          } else {
              // TODO we can expand this later to make more sense
              return Err(Error::MissingClientSyncState);
          }
      }
*/

fn process_genesis_trigger_block(
    block_mf: &L1BlockManifest,
    params: &RollupParams,
) -> Result<InternalState, Error> {
    // TODO maybe more bookkeeping?
    Ok(InternalState::new(block_mf.block_hash(), None))
}

fn process_l1_block(
    state: &InternalState,
    height: u64,
    block_mf: &L1BlockManifest,
    params: &RollupParams,
) -> Result<InternalState, Error> {
    let blkid = block_mf.block_hash();
    let mut checkpoint = state.last_checkpoint().cloned();

    // Iterate through all of the protocol operations in all of the txs.
    // TODO split out each proto op handling into a separate function
    for txs in block_mf.txs() {
        for op in txs.protocol_ops() {
            match op {
                ProtocolOperation::Checkpoint(signed_ckpt) => {
                    let ckpt = signed_ckpt.checkpoint();

                    // Before we do anything, make sure that the checkpoint
                    // proof is correct.  There's no point in looking at it more
                    // if the proof is invalid.
                    // TODO update this proof checking to use simplified
                    // interface, see comment in checkpoint_verification
                    let receipt = signed_ckpt.checkpoint().get_proof_receipt();
                    if !verify_proof(ckpt, &receipt, params).is_ok() {
                        // If it's invalid then just print a warning and move on.
                        warn!(%height, "ignoring invalid checkpoint in L1 block");
                        continue;
                    }

                    // If we had a previous checkpoint, verify it extends from it.
                    if let Some(prev_ckpt) = &checkpoint {
                        if !check_checkpoint_extends(ckpt, prev_ckpt, params) {
                            warn!(%height, "ignoring noncontinuous checkpoint in L1 block");
                            continue;
                        }
                    } else {
                        // If not, then it should be for the genesis epoch.
                        if ckpt.batch_info().epoch != 0 {
                            continue;
                        }
                    }

                    let l1ckpt = L1Checkpoint::new(
                        ckpt.batch_info().clone(),
                        ckpt.batch_transition().clone(),
                        ckpt.base_state_commitment().clone(),
                        !ckpt.proof().is_empty(),
                        height,
                    );

                    // If it all looks good then overwrite the saved checkpoint.
                    checkpoint = Some(l1ckpt);
                }

                // The rest we don't care about here.  Maybe we will in the
                // future, like for when we actually do DA, but not for now.
                _ => {}
            }
        }
    }

    Ok(InternalState::new(blkid, checkpoint))
}

fn check_checkpoint_extends(
    checkpoint: &Checkpoint,
    prev: &L1Checkpoint,
    params: &RollupParams,
) -> bool {
    let last_l1_tsn = prev.batch_transition.l1_transition;
    let last_l2_tsn = prev.batch_transition.l2_transition;
    let l1_tsn = checkpoint.batch_transition().l1_transition;
    let l2_tsn = checkpoint.batch_transition().l2_transition;

    // Check that the L1 blocks match up.
    if l1_tsn.0 != last_l1_tsn.1 {
        warn!("checkpoint mismatch on L1 state!");
        return false;
    }

    if l2_tsn.0 != last_l2_tsn.1 {
        warn!("checkpoint mismatch on L2 state!");
        return false;
    }

    true
}

// TODO remove this old code after we've reconsolidated its responsibilities
/*SyncEvent::NewTipBlock(blkid) => {
    // TODO remove ^this sync event type and all associated fields
    debug!(?blkid, "Received NewTipBlock");
    let block = context.get_l2_block_data(blkid)?;

    // TODO: get chainstate idx from blkid OR pass correct idx in sync event
    let slot = block.header().blockidx();
    let chainstate = context.get_toplevel_chainstate(slot)?;

    debug!(?chainstate, "Chainstate for new tip block");
    // height of last matured L1 block in chain state
    let chs_last_buried = chainstate.l1_view().safe_height().saturating_sub(1);
    // buried height in client state
    let cls_last_buried = state.state().l1_view().buried_l1_height();

    if chs_last_buried > cls_last_buried {
        // can bury till last matured block in chainstate
        // FIXME: this logic is not necessary for fullnode.
        // Need to refactor this part for block builder only.
        let client_state_bury_height = min(
            chs_last_buried,
            // keep at least 1 item
            state.state().l1_view().tip_height().saturating_sub(1),
        );

        state.update_buried(client_state_bury_height);
    }

    // TODO better checks here
    state.accept_l2_block(*blkid, block.block().header().blockidx());
    state.push_action(SyncAction::UpdateTip(*blkid));

    handle_checkpoint_finalization(state, blkid, params, context)?;
}*/

/// Handles the maturation of L1 height by finalizing checkpoints and emitting
/// sync actions.
///
/// This function checks if there are any verified checkpoints at or before the
/// given `maturable_height`. If such checkpoints exist, it attempts to
/// finalize them by checking if the corresponding L2 block is available in the
/// L2 database. If the L2 block is found, it marks the checkpoint as finalized
/// and emits a sync action to finalize the L2 block. If the L2 block is not
/// found, it logs a warning and skips the finalization.
///
/// # Arguments
///
/// * `state` - A reference to the current client state.
/// * `maturable_height` - The height at which L1 blocks are considered mature.
/// * `database` - A reference to the database interface.
///
/// # Returns
///
/// A tuple containing:
/// * A vector of [`ClientStateWrite`] representing the state changes to be written.
/// * A vector of [`SyncAction`] representing the actions to be synchronized.
fn handle_mature_l1_height(
    state: &mut ClientStateMut,
    maturable_height: u64,
    context: &impl EventContext,
) -> Result<(), Error> {
    // If there are no checkpoints then return early.
    if !state
        .state()
        .has_verified_checkpoint_before(maturable_height)
    {
        return Ok(());
    }

    // If there *are* checkpoints at or before the maturable height, mark them
    // as finalized
    if let Some(checkpt) = state
        .state()
        .get_last_verified_checkpoint_before(maturable_height)
    {
        // FinalizeBlock Should only be applied when l2_block is actually
        // available in l2_db
        // If l2 blocks is not in db then finalization will happen when
        // l2Block is fetched from the network and the corresponding
        //checkpoint is already finalized.
        let epoch = checkpt.batch_info.get_epoch_commitment();
        let blkid = *epoch.last_blkid();

        match context.get_l2_block_data(&blkid) {
            Ok(_) => {
                // Emit sync action for finalizing an epoch
                trace!(%maturable_height, %blkid, "epoch terminal block found in DB, emitting FinalizedEpoch action");
                state.push_action(SyncAction::FinalizeEpoch(epoch));
            }

            // TODO figure out how to make this not matter
            Err(Error::MissingL2Block(_)) => {
                warn!(
                    %maturable_height, ?epoch, "epoch terminal not in DB yet, skipping finalization"
                );
            }

            Err(e) => {
                error!(%blkid, err = %e, "error while checking for block present");
                return Err(e.into());
            }
        }
    } else {
        warn!(
        %maturable_height,
        "expected to find blockid corresponding to buried l1 height in confirmed_blocks but could not find"
        );
    }

    Ok(())
}

// TODO most of this is irrelevant now and can just be removed
/*
/// Handles the finalization of a checkpoint by processing the corresponding L2
/// block ID.
///
/// This function checks if the given L2 block ID corresponds to a verified
/// checkpoint. If it does, it calculates the maturable height based on the
/// rollup parameters and the current state. If the L1 height associated with
/// the L2 block ID is less than the maturable height, it calls
/// [`handle_mature_l1_height`] and returns writes and sync actions.
///
/// # Arguments
///
/// * `state` - A reference to the current client state.
/// * `blkid` - A reference to the L2 block ID to be finalized.
/// * `params` - A reference to the rollup parameters.
/// * `database` - A reference to the database interface.
///
/// # Returns
///
/// A tuple containing:
/// * A vector of [`ClientStateWrite`] representing the state changes to be written.
/// * A vector of [`SyncAction`] representing the actions to be synchronized.
fn handle_checkpoint_finalization(
    state: &mut ClientStateMut,
    blkid: &L2BlockId,
    params: &Params,
    context: &impl EventContext,
) -> Result<(), Error> {
    let verified_checkpoints: &[L1Checkpoint] = state.state().l1_view().verified_checkpoints();
    match find_l1_height_for_l2_blockid(verified_checkpoints, blkid) {
        Some(l1_height) => {
            let safe_depth = params.rollup().l1_reorg_safe_depth as u64;

            // Maturable height is the height at which l1 blocks are sufficiently buried
            // and have negligible chance of reorg.
            let maturable_height = state.state().next_exp_l1_block().saturating_sub(safe_depth);

            // The l1 height should be handled only if it is less than maturable height
            if l1_height < maturable_height {
                handle_mature_l1_height(state, l1_height, context)?;
            }
        }
        None => {
            debug!(%blkid, "L2 block not found in verified checkpoints, possibly not a last block in the checkpoint.");
        }
    }

    Ok(())
}
*/

/// Searches for a given [`L2BlockId`] within a slice of [`L1Checkpoint`] structs
/// and returns the height of the corresponding L1 block if found.
fn find_l1_height_for_l2_blockid(
    checkpoints: &[L1Checkpoint],
    target_l2_blockid: &L2BlockId,
) -> Option<u64> {
    checkpoints
        .binary_search_by(|checkpoint| {
            checkpoint
                .batch_info
                .final_l2_blockid()
                .cmp(target_l2_blockid)
        })
        .ok()
        .map(|index| checkpoints[index].height)
}

// TODO this is being incrementally moved over to checkpoint_verification
/*
/// Filters a list of [`BatchCheckpoint`]s, returning only those that form a valid sequence
/// of checkpoints.
///
/// A valid checkpoint is one whose proof passes verification, and its index follows
/// sequentially from the previous valid checkpoint.
///
/// # Arguments
///
/// * `state` - The client's current state, which provides the L1 view and pending checkpoints.
/// * `checkpoints` - A slice of [`L1CommittedCheckpoint`]s to be filtered.
/// * `params` - Parameters required for verifying checkpoint proofs.
///
/// # Returns
///
/// A vector containing the valid sequence of [`Checkpoint`]s, starting from the first valid
/// one.
pub fn filter_verified_checkpoints(
    state: &ClientState,
    checkpoints: &[L1CommittedCheckpoint],
    params: &RollupParams,
) -> Vec<L1CommittedCheckpoint> {
    let l1_view = state.l1_view();
    let last_verified = l1_view.verified_checkpoints().last();
    let last_finalized = l1_view.last_finalized_checkpoint();

    let (mut expected_idx, mut last_valid_checkpoint) = if last_verified.is_some() {
        last_verified
    } else {
        last_finalized
    }
    .map(|x| (x.batch_info.epoch() + 1, Some(&x.batch_transition)))
    .unwrap_or((0, None)); // expect the first checkpoint

    let mut result_checkpoints = Vec::new();

    for checkpoint in checkpoints {
        let curr_idx = checkpoint.checkpoint.batch_info().epoch;
        let proof_receipt: ProofReceipt = checkpoint.checkpoint.get_proof_receipt();
        if curr_idx != expected_idx {
            warn!(%expected_idx, %curr_idx, "Received invalid checkpoint idx, ignoring.");
            continue;
        }
        if expected_idx == 0 && verify_proof(&checkpoint.checkpoint, &proof_receipt, params).is_ok()
        {
            result_checkpoints.push(checkpoint.clone());
            last_valid_checkpoint = Some(checkpoint.checkpoint.batch_transition());
        } else if expected_idx == 0 {
            warn!(%expected_idx, "Received invalid checkpoint proof, ignoring.");
        } else {
            let last_l1_tsn = last_valid_checkpoint
                .expect("There should be a last_valid_checkpoint")
                .l1_transition;
            let last_l2_tsn = last_valid_checkpoint
                .expect("There should be a last_valid_checkpoint")
                .l2_transition;
            let l1_tsn = checkpoint.checkpoint.batch_transition().l1_transition;
            let l2_tsn = checkpoint.checkpoint.batch_transition().l2_transition;

            if l1_tsn.0 != last_l1_tsn.1 {
                warn!(obtained = ?l1_tsn.0, expected = ?last_l1_tsn.1, "Received invalid checkpoint l1 transition, ignoring.");
                continue;
            }
            if l2_tsn.0 != last_l2_tsn.1 {
                warn!(obtained = ?l2_tsn.0, expected = ?last_l2_tsn.1, "Received invalid checkpoint l2 transition, ignoring.");
                continue;
            }
            if verify_proof(&checkpoint.checkpoint, &proof_receipt, params).is_ok() {
                result_checkpoints.push(checkpoint.clone());
                last_valid_checkpoint = Some(checkpoint.checkpoint.batch_transition());
            } else {
                warn!(%expected_idx, "Received invalid checkpoint proof, ignoring.");
                continue;
            }
        }
    }

    result_checkpoints
}
*/

#[cfg(test)]
mod tests {
    use bitcoin::params::MAINNET;
    use strata_db::traits::L1Database;
    use strata_primitives::{block_credential, l1::L1BlockRecord};
    use strata_rocksdb::test_utils::get_common_db;
    use strata_state::{l1::L1BlockId, operation};
    use strata_test_utils::{
        bitcoin::{gen_l1_chain, get_btc_chain, BtcChainSegment},
        l2::{gen_client_state, gen_params},
        ArbitraryGenerator,
    };

    use super::*;
    use crate::genesis;

    pub struct DummyEventContext {
        chainseg: BtcChainSegment,
    }

    impl DummyEventContext {
        pub fn new() -> Self {
            Self {
                chainseg: get_btc_chain(),
            }
        }
    }

    impl EventContext for DummyEventContext {
        fn get_l1_block_manifest(&self, height: u64) -> Result<L1BlockManifest, Error> {
            let rec = self.chainseg.get_block_record(height as u32);
            Ok(L1BlockManifest::new(rec, height))
        }

        fn get_l2_block_data(&self, blkid: &L2BlockId) -> Result<L2BlockBundle, Error> {
            Err(Error::MissingL2Block(*blkid))
        }

        fn get_toplevel_chainstate(&self, slot: u64) -> Result<Chainstate, Error> {
            Err(Error::MissingIdxChainstate(slot))
        }
    }

    struct TestEvent<'a> {
        event: SyncEvent,
        expected_actions: &'a [SyncAction],
    }

    struct TestCase<'a> {
        description: &'static str,
        events: &'a [TestEvent<'a>], // List of events to process
        state_assertions: Box<dyn Fn(&ClientState)>, // Closure to verify state after all events
    }

    fn run_test_cases(test_cases: &[TestCase], state: &mut ClientState, params: &Params) {
        let context = DummyEventContext::new();

        for case in test_cases {
            println!("Running test case: {}", case.description);

            let mut outputs = Vec::new();
            for (i, test_event) in case.events.iter().enumerate() {
                let mut state_mut = ClientStateMut::new(state.clone());
                process_event(&mut state_mut, &test_event.event, &context, params).unwrap();
                let output = state_mut.into_update();
                outputs.push(output.clone());

                assert_eq!(
                    output.actions(),
                    test_event.expected_actions,
                    "Failed on actions for event {} in test case: {}",
                    i + 1,
                    case.description
                );

                *state = output.into_state();
            }

            // Run the state assertions after all events
            (case.state_assertions)(state);
        }
    }

    #[test]
    fn test_genesis() {
        let params = gen_params();
        let mut state = gen_client_state(Some(&params));

        let horizon = params.rollup().horizon_l1_height;
        let genesis = params.rollup().genesis_l1_height;

        let chain = get_btc_chain();
        let l1_verification_state =
            chain.get_verification_state(genesis as u32 + 1, &MAINNET.clone().into());

        let genesis_block = genesis::make_genesis_block(&params);
        let genesis_blockid = genesis_block.header().get_blockid();
        let l1_chain = chain.get_block_records(horizon as u32, 10);
        let blkids: Vec<L1BlockId> = l1_chain.iter().map(|b| b.block_hash()).collect();

        let test_cases = [
            TestCase {
                description: "At horizon block",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(horizon, l1_chain[0].block_hash()),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[0].block_hash())
                        );
                        assert_eq!(state.next_exp_l1_block(), horizon + 1);
                    }
                }),
            },
            TestCase {
                description: "At horizon block + 1",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(horizon + 1, l1_chain[1].block_hash()),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[1].block_hash())
                        );
                        // Because values for horizon is 40318, genesis is 40320
                        assert_eq!(state.next_exp_l1_block(), genesis);
                    }
                }),
            },
            TestCase {
                description: "As the genesis of L2 is reached but not locked in yet",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(
                        genesis,
                        l1_chain[(genesis - horizon) as usize].block_hash(),
                    ),
                    expected_actions: &[],
                }],
                state_assertions: Box::new(move |state| {
                    assert!(!state.is_chain_active());
                    assert_eq!(state.next_exp_l1_block(), genesis + 1);
                }),
            },
            TestCase {
                description: "At genesis + 1",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(
                        genesis + 1,
                        l1_chain[(genesis + 1 - horizon) as usize].block_hash(),
                    ),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    let blkids = blkids.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[(genesis + 1 - horizon) as usize].block_hash(),)
                        );
                        assert_eq!(state.next_exp_l1_block(), genesis + 2);
                        assert_eq!(
                            state.l1_view().local_unaccepted_blocks(),
                            &blkids[0..(genesis + 1 - horizon + 1) as usize]
                        );
                    }
                }),
            },
            TestCase {
                description: "At genesis + 2",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(
                        genesis + 2,
                        l1_chain[(genesis + 2 - horizon) as usize].block_hash(),
                    ),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    let blkids = blkids.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[(genesis + 2 - horizon) as usize].block_hash())
                        );
                        assert_eq!(state.next_exp_l1_block(), genesis + 3);
                        assert_eq!(
                            state.l1_view().local_unaccepted_blocks(),
                            &blkids[0..(genesis + 2 - horizon + 1) as usize]
                        );
                    }
                }),
            },
            TestCase {
                description: "At genesis + 3, lock in genesis",
                events: &[
                    TestEvent {
                        event: SyncEvent::L1BlockGenesis(
                            genesis + 3,
                            l1_verification_state.clone(),
                        ),
                        expected_actions: &[SyncAction::L2Genesis(
                            l1_chain[(genesis - horizon) as usize].block_hash(),
                        )],
                    },
                    TestEvent {
                        event: SyncEvent::L1Block(
                            genesis + 3,
                            l1_chain[(genesis + 3 - horizon) as usize].block_hash(),
                        ),
                        expected_actions: &[],
                    },
                ],
                state_assertions: Box::new({
                    let l1_chain = &l1_chain;
                    move |state| {
                        assert!(state.is_chain_active());
                        assert_eq!(state.next_exp_l1_block(), genesis + 4);
                    }
                }),
            },
            TestCase {
                description: "Rollback to genesis height",
                events: &[TestEvent {
                    event: SyncEvent::L1Revert(genesis),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({ move |state| {} }),
            },
        ];

        run_test_cases(&test_cases, &mut state, &params);
    }
}
