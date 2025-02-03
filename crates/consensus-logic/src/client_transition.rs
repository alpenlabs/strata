//! Core state transition function.
#![allow(unused)] // still under development

use std::cmp::min;

use bitcoin::{block::Header, consensus::deserialize};
use strata_db::traits::{
    ChainstateProvider, Database, L1DataProvider, L2DataProvider, L2DataStore,
};
use strata_primitives::prelude::*;
use strata_state::{
    batch::{BatchCheckpoint, BatchInfo},
    block,
    client_state::*,
    header::L2Header,
    id::L2BlockId,
    l1::{get_btc_params, HeaderVerificationState, L1BlockId},
    operation::*,
    sync_event::SyncEvent,
};
use tracing::*;

use crate::{errors::*, genesis::make_genesis_block, l1_handler::verify_proof};

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
            // If the block is before the horizon we don't care about it.
            if *height < params.rollup().horizon_l1_height {
                #[cfg(test)]
                eprintln!("early L1 block at h={height}, you may have set up the test env wrong");

                warn!(%height, "ignoring unexpected L1Block event before horizon");
                return Ok(ClientUpdateOutput::new(writes, actions));
            }

            // FIXME this doesn't do any SPV checks to make sure we only go to
            // a longer chain, it just does it unconditionally
            let l1prov = database.l1_provider();
            let block_mf = l1prov
                .get_block_manifest(*height)?
                .ok_or(Error::MissingL1BlockHeight(*height))?;

            let l1v = state.l1_view();
            let l1_vs = state.l1_view().tip_verification_state();

            // Do the consensus checks
            if let Some(l1_vs) = l1_vs {
                let l1_vs_height = l1_vs.last_verified_block_num as u64;
                let mut updated_l1vs = l1_vs.clone();

                for block_height in (l1_vs_height..*height) {
                    let block_mf = l1prov
                        .get_block_manifest(block_height)?
                        .ok_or(Error::MissingL1BlockHeight(block_height))?;
                    let header: Header = deserialize(block_mf.header()).unwrap();
                    updated_l1vs =
                        updated_l1vs.check_and_update_continuity_new(&header, &get_btc_params());
                }
                writes.push(ClientStateWrite::UpdateVerificationState(updated_l1vs))
            }

            // Only accept the block if it's the next block in the chain we expect to accept.
            let cur_seen_tip_height = l1v.tip_height();
            let next_exp_height = l1v.next_expected_block();
            if next_exp_height > params.rollup().horizon_l1_height {
                // TODO check that the new block we're trying to add has the same parent as the tip
                // block
                let cur_tip_block = l1prov
                    .get_block_manifest(cur_seen_tip_height)?
                    .ok_or(Error::MissingL1BlockHeight(cur_seen_tip_height))?;
            }

            if *height == next_exp_height {
                writes.push(ClientStateWrite::AcceptL1Block(*l1blkid));
            } else {
                #[cfg(test)]
                eprintln!("not sure what to do here h={height} exp={next_exp_height}");
                return Err(Error::OutOfOrderL1Block(next_exp_height, *height, *l1blkid));
            }

            // If we have some number of L1 blocks finalized, also emit an `UpdateBuried` write.
            let safe_depth = params.rollup().l1_reorg_safe_depth as u64;
            let maturable_height = next_exp_height.saturating_sub(safe_depth);

            if maturable_height > params.rollup().horizon_l1_height && state.is_chain_active() {
                let (wrs, acts) = handle_mature_l1_height(maturable_height, state, database);
                writes.extend(wrs);
                actions.extend(acts);
            }
        }

        SyncEvent::L1BlockGenesis(height, l1_verification_state) => {
            debug!(%height, "Received L1BlockGenesis");
            let horizon_ht = params.rollup.horizon_l1_height;
            let genesis_ht = params.rollup.genesis_l1_height;

            let state_ht = l1_verification_state.last_verified_block_num as u64;
            if genesis_ht != state_ht {
                let error_msg = format!(
                    "Expected height: {} Found height: {} in state",
                    genesis_ht, state_ht
                );
                return Err(Error::GenesisFailed(error_msg));
            }

            let threshold = params.rollup.l1_reorg_safe_depth;
            let genesis_threshold = genesis_ht + threshold as u64;

            debug!(%genesis_threshold, %genesis_ht, active=%state.is_chain_active(), "Inside activate chain");

            // If necessary, activate the chain!
            if !state.is_chain_active() && *height >= genesis_threshold {
                debug!("emitting chain activation");
                let genesis_block = make_genesis_block(params);

                writes.push(ClientStateWrite::ActivateChain);
                writes.push(ClientStateWrite::UpdateVerificationState(
                    l1_verification_state.clone(),
                ));
                writes.push(ClientStateWrite::ReplaceSync(Box::new(
                    SyncState::from_genesis_blkid(genesis_block.header().get_blockid()),
                )));
                actions.push(SyncAction::L2Genesis(
                    l1_verification_state.last_verified_block_hash,
                ));
            }
        }

        SyncEvent::L1Revert(to_height) => {
            let l1prov = database.l1_provider();

            let buried = state.l1_view().buried_l1_height();
            if *to_height < buried {
                error!(%to_height, %buried, "got L1 revert below buried height");
                return Err(Error::ReorgTooDeep(*to_height, buried));
            }

            writes.push(ClientStateWrite::RollbackL1BlocksTo(*to_height));
        }

        SyncEvent::L1DABatch(height, checkpoints) => {
            debug!(%height, "received L1DABatch");

            if let Some(ss) = state.sync() {
                // TODO load it up and figure out what's there, see if we have to
                // load the state updates from L1 or something
                let l2prov = database.l2_provider();

                let proof_verified_checkpoints =
                    filter_verified_checkpoints(state, checkpoints, params.rollup());

                // When DABatch appears, it is only confirmed at the moment. These will be finalized
                // only when the corresponding L1 block is buried enough
                writes.push(ClientStateWrite::CheckpointsReceived(
                    checkpoints
                        .iter()
                        .map(|x| {
                            L1Checkpoint::new(
                                x.batch_info().clone(),
                                x.bootstrap_state().clone(),
                                !x.proof().is_empty(),
                                *height,
                            )
                        })
                        .collect(),
                ));

                actions.push(SyncAction::WriteCheckpoints(
                    *height,
                    proof_verified_checkpoints,
                ));
            } else {
                // TODO we can expand this later to make more sense
                return Err(Error::MissingClientSyncState);
            }
        }

        SyncEvent::NewTipBlock(blkid) => {
            debug!(?blkid, "Received NewTipBlock");
            let l2prov = database.l2_provider();
            let block = l2prov
                .get_block_data(*blkid)?
                .ok_or(Error::MissingL2Block(*blkid))?;

            // TODO: get chainstate idx from blkid OR pass correct idx in sync event
            let block_idx = block.header().blockidx();
            let chainstate_provider = database.chain_state_provider();
            let chainstate = chainstate_provider
                .get_toplevel_state(block_idx)?
                .ok_or(Error::MissingIdxChainstate(block_idx))?;

            debug!(?chainstate, "Chainstate for new tip block");
            // height of last matured L1 block in chain state
            let chs_last_buried = chainstate.l1_view().safe_height().saturating_sub(1);
            // buried height in client state
            let cls_last_buried = state.l1_view().buried_l1_height();

            if chs_last_buried > cls_last_buried {
                // can bury till last matured block in chainstate
                // FIXME: this logic is not necessary for fullnode.
                // Need to refactor this part for block builder only.
                let client_state_bury_height = min(
                    chs_last_buried,
                    // keep at least 1 item
                    state.l1_view().tip_height().saturating_sub(1),
                );
                writes.push(ClientStateWrite::UpdateBuried(client_state_bury_height));
            }

            // TODO better checks here
            writes.push(ClientStateWrite::AcceptL2Block(
                *blkid,
                block.block().header().blockidx(),
            ));
            actions.push(SyncAction::UpdateTip(*blkid));

            let (wrs, acts) = handle_checkpoint_finalization(state, blkid, params, database);
            writes.extend(wrs);
            actions.extend(acts);
        }
    }

    Ok(ClientUpdateOutput::new(writes, actions))
}

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
/// * `maturable_height` - The height at which L1 blocks are considered mature.
/// * `state` - A reference to the current client state.
/// * `database` - A reference to the database interface.
///
/// # Returns
///
/// A tuple containing:
/// * A vector of [`ClientStateWrite`] representing the state changes to be written.
/// * A vector of [`SyncAction`] representing the actions to be synchronized.
fn handle_mature_l1_height(
    maturable_height: u64,
    state: &ClientState,
    database: &impl Database,
) -> (Vec<ClientStateWrite>, Vec<SyncAction>) {
    let mut writes = Vec::new();
    let mut actions = Vec::new();

    // If there are checkpoints at or before the maturable height, mark them as finalized
    if state
        .l1_view()
        .has_verified_checkpoint_before(maturable_height)
    {
        if let Some(checkpt) = state
            .l1_view()
            .get_last_verified_checkpoint_before(maturable_height)
        {
            // FinalizeBlock Should only be applied when l2_block is actually
            // available in l2_db
            // If l2 blocks is not in db then finalization will happen when
            // l2Block is fetched from the network and the corresponding
            //checkpoint is already finalized.
            let l2_blockid = checkpt.batch_info.l2_blockid;

            match database.l2_provider().get_block_data(l2_blockid) {
                Ok(Some(bundle)) => {
                    let block_height = bundle.header().blockidx();
                    // FIXME: only checking height and not for potential fork
                    match database.chain_state_provider().get_last_state_idx() {
                        Ok(chainstate_height) => {
                            if block_height > chainstate_height {
                                warn!(%maturable_height, %l2_blockid, "l2 block not in chainstate yet, skipping finalize");
                            } else {
                                debug!(%maturable_height, "Writing CheckpointFinalized");
                                writes
                                    .push(ClientStateWrite::CheckpointFinalized(maturable_height));
                                // Emit sync action for finalizing a l2 block
                                info!(%maturable_height, %l2_blockid, "l2 block found in db, push FinalizeBlock SyncAction");
                                actions.push(SyncAction::FinalizeBlock(l2_blockid));
                            }
                        }
                        Err(e) => {
                            error!(%e, "error while fetching block data from chainstate db");
                        }
                    }
                }
                Ok(None) => {
                    warn!(
                        %maturable_height,%l2_blockid, "l2 block not in db yet, skipping finalize"
                    );
                }
                Err(e) => {
                    error!(%e, "error while fetching block data from l2_db");
                }
            }
        } else {
            warn!(
            %maturable_height,
            "expected to find blockid corresponding to buried l1 height in confirmed_blocks but could not find"
            );
        }
    }
    (writes, actions)
}

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
    state: &ClientState,
    blkid: &L2BlockId,
    params: &Params,
    database: &impl Database,
) -> (Vec<ClientStateWrite>, Vec<SyncAction>) {
    let mut writes = Vec::new();
    let mut actions = Vec::new();
    let verified_checkpoints: &[L1Checkpoint] = state.l1_view().verified_checkpoints();
    match find_l1_height_for_l2_blockid(verified_checkpoints, blkid) {
        Some(l1_height) => {
            let safe_depth = params.rollup().l1_reorg_safe_depth as u64;

            // Maturable height is the height at which l1 blocks are sufficiently buried
            // and have negligible chance of reorg.
            let maturable_height = state
                .l1_view()
                .next_expected_block()
                .saturating_sub(safe_depth);

            // The l1 height should be handled only if it is less than maturable height
            if l1_height < maturable_height {
                let (wrs, acts) = handle_mature_l1_height(l1_height, state, database);
                writes.extend(wrs);
                actions.extend(acts);
            }
        }
        None => {
            debug!(%blkid, "L2 block not found in verified checkpoints, possibly not a last block in the checkpoint.");
        }
    }
    (writes, actions)
}

/// Searches for a given [`L2BlockId`] within a slice of [`L1Checkpoint`] structs
/// and returns the height of the corresponding L1 block if found.
fn find_l1_height_for_l2_blockid(
    checkpoints: &[L1Checkpoint],
    target_l2_blockid: &L2BlockId,
) -> Option<u64> {
    checkpoints
        .binary_search_by(|checkpoint| checkpoint.batch_info.l2_blockid.cmp(target_l2_blockid))
        .ok()
        .map(|index| checkpoints[index].height)
}

/// Filters a list of [`BatchCheckpoint`]s, returning only those that form a valid sequence
/// of checkpoints.
///
/// A valid checkpoint is one whose proof passes verification, and its index follows
/// sequentially from the previous valid checkpoint.
///
/// # Arguments
///
/// * `state` - The client's current state, which provides the L1 view and pending checkpoints.
/// * `checkpoints` - A slice of [`BatchCheckpoint`]s to be filtered.
/// * `params` - Parameters required for verifying checkpoint proofs.
///
/// # Returns
///
/// A vector containing the valid sequence of [`BatchCheckpoint`]s, starting from the first valid
/// one.
pub fn filter_verified_checkpoints(
    state: &ClientState,
    checkpoints: &[BatchCheckpoint],
    params: &RollupParams,
) -> Vec<BatchCheckpoint> {
    let l1_view = state.l1_view();
    let last_verified = l1_view.verified_checkpoints().last();
    let last_finalized = l1_view.last_finalized_checkpoint();

    let (mut expected_idx, mut last_valid_checkpoint) = if last_verified.is_some() {
        last_verified
    } else {
        last_finalized
    }
    .map(|x| (x.batch_info.idx() + 1, Some(&x.batch_info)))
    .unwrap_or((0, None)); // expect the first checkpoint

    let mut result_checkpoints = Vec::new();

    for checkpoint in checkpoints {
        let curr_idx = checkpoint.batch_info().idx;
        if curr_idx != expected_idx {
            warn!(%expected_idx, %curr_idx, "Received invalid checkpoint idx, ignoring.");
            continue;
        }
        if expected_idx == 0 && verify_proof(checkpoint, params).is_ok() {
            result_checkpoints.push(checkpoint.clone());
            last_valid_checkpoint = Some(checkpoint.batch_info());
        } else if expected_idx == 0 {
            warn!(%expected_idx, "Received invalid checkpoint proof, ignoring.");
        } else {
            let last_l1_tsn = last_valid_checkpoint
                .expect("There should be a last_valid_checkpoint")
                .l1_transition;
            let last_l2_tsn = last_valid_checkpoint
                .expect("There should be a last_valid_checkpoint")
                .l2_transition;
            let l1_tsn = checkpoint.batch_info().l1_transition;
            let l2_tsn = checkpoint.batch_info().l2_transition;

            if l1_tsn.0 == last_l1_tsn.1 {
                warn!(obtained = ?l1_tsn.0, expected = ?last_l1_tsn.1, "Received invalid checkpoint l1 transition, ignoring.");
                continue;
            }
            if l2_tsn.0 == last_l2_tsn.1 {
                warn!(obtained = ?l2_tsn.0, expected = ?last_l2_tsn.1, "Received invalid checkpoint l2 transition, ignoring.");
                continue;
            }
            if verify_proof(checkpoint, params).is_ok() {
                result_checkpoints.push(checkpoint.clone());
                last_valid_checkpoint = Some(checkpoint.batch_info());
            } else {
                warn!(%expected_idx, "Received invalid checkpoint proof, ignoring.");
                continue;
            }
        }
    }

    result_checkpoints
}

#[cfg(test)]
mod tests {
    use bitcoin::params::MAINNET;
    use strata_db::traits::L1DataStore;
    use strata_primitives::{block_credential, l1::L1BlockManifest};
    use strata_rocksdb::test_utils::get_common_db;
    use strata_state::{l1::L1BlockId, operation};
    use strata_test_utils::{
        bitcoin::{gen_l1_chain, get_btc_chain},
        l2::{gen_client_state, gen_params},
        ArbitraryGenerator,
    };

    use super::*;
    use crate::genesis;

    #[test]
    fn test_genesis() {
        // TODO there's a ton of duplication in this test, we could wrap it up so we just run
        // through a table and call a function with it

        let database = get_common_db();
        let params = gen_params();
        let mut state = gen_client_state(Some(&params));

        assert!(!state.is_chain_active());

        let horizon = params.rollup().horizon_l1_height;
        let genesis = params.rollup().genesis_l1_height;

        let chain = get_btc_chain();
        let l1_chain = chain.get_block_manifests(horizon as u32, 10);
        let l1_verification_state =
            chain.get_verification_state(genesis as u32 + 1, &MAINNET.clone().into());

        for (i, b) in l1_chain.iter().enumerate() {
            let l1_store = database.l1_store();
            l1_store
                .put_block_data(i as u64 + horizon, b.clone(), Vec::new())
                .expect("test: insert blocks");
        }

        let blkids: Vec<L1BlockId> = l1_chain.iter().map(|b| b.block_hash().into()).collect();

        // at horizon block
        {
            let height = horizon;
            let idx = (height - horizon) as usize;
            let l1_block_id = l1_chain[idx].block_hash().into();
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let writes = output.writes();
            let actions = output.actions();

            let expected_writes = [ClientStateWrite::AcceptL1Block(l1_block_id)];
            let expected_actions = [];

            assert_eq!(writes, expected_writes);
            assert_eq!(actions, expected_actions);

            operation::apply_writes_to_state(&mut state, writes.iter().cloned());

            assert!(!state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), horizon + 1);
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[idx..idx + 1]
            );
        }

        // at horizon block + 1
        {
            let height = params.rollup().horizon_l1_height + 1;
            let idx = height - horizon;
            let l1_block_id = l1_chain[idx as usize].block_hash().into();
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let writes = output.writes();
            let actions = output.actions();

            let expected_writes = [ClientStateWrite::AcceptL1Block(l1_block_id)];
            let expected_actions = [];

            assert_eq!(writes, expected_writes);
            assert_eq!(actions, expected_actions);

            operation::apply_writes_to_state(&mut state, writes.iter().cloned());

            assert!(!state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), genesis);
            assert_eq!(state.l1_view().local_unaccepted_blocks(), &blkids[0..2]);
        }

        // as the genesis of L2 is reached, but not locked in yet
        {
            let height = genesis;
            let idx = (height - horizon) as usize;
            let l1_block_id = l1_chain[idx].block_hash().into();
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let expected_writes = [ClientStateWrite::AcceptL1Block(l1_block_id)];
            let expected_actions = [];

            assert_eq!(output.writes(), expected_writes);
            assert_eq!(output.actions(), expected_actions);

            operation::apply_writes_to_state(&mut state, output.writes().iter().cloned());

            assert!(!state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), height + 1);
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[0..idx + 1]
            );
        }

        // genesis + 1
        {
            let height = genesis + 1;
            let idx = (height - horizon) as usize;
            let l1_block_id = l1_chain[idx].block_hash().into();
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let expected_writes = [ClientStateWrite::AcceptL1Block(l1_block_id)];
            let expected_actions = [];

            assert_eq!(output.writes(), expected_writes);
            assert_eq!(output.actions(), expected_actions);

            operation::apply_writes_to_state(&mut state, output.writes().iter().cloned());

            assert!(!state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), height + 1);
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[0..idx + 1]
            );
        }

        // genesis + 2
        {
            let height = genesis + 2;
            let idx = (height - horizon) as usize;
            let l1_block_id = l1_chain[idx].block_hash().into();
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let expected_writes = [ClientStateWrite::AcceptL1Block(l1_block_id)];
            let expected_actions = [];

            assert_eq!(output.writes(), expected_writes);
            assert_eq!(output.actions(), expected_actions);

            operation::apply_writes_to_state(&mut state, output.writes().iter().cloned());

            assert!(!state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), height + 1);
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[0..idx + 1]
            );
        }

        // genesis + 3, where we should lock in genesis
        {
            let height = genesis + 3;

            let idx = (height - horizon) as usize;
            let genesis_id = l1_chain[(genesis - horizon) as usize].block_hash().into();
            let l1_block_id = l1_chain[idx as usize].block_hash().into();

            let event1 = SyncEvent::L1BlockGenesis(height, l1_verification_state.clone());
            let event2 = SyncEvent::L1Block(height, l1_block_id);

            let output1 = process_event(&state, &event1, database.as_ref(), &params).unwrap();
            let output2 = process_event(&state, &event2, database.as_ref(), &params).unwrap();

            let genesis_block = genesis::make_genesis_block(&params);
            let genesis_blockid = genesis_block.header().get_blockid();

            let expected_writes1 = [
                ClientStateWrite::ActivateChain,
                ClientStateWrite::UpdateVerificationState(l1_verification_state.clone()),
                ClientStateWrite::ReplaceSync(Box::new(SyncState::from_genesis_blkid(
                    genesis_blockid,
                ))),
            ];
            let expected_writes2 = [ClientStateWrite::AcceptL1Block(l1_block_id)];

            let expected_actions1 = [SyncAction::L2Genesis(genesis_id)];
            let expected_actions2 = [];

            assert_eq!(output1.writes(), expected_writes1);
            assert_eq!(output1.actions(), expected_actions1);

            assert_eq!(output2.writes(), expected_writes2);
            assert_eq!(output2.actions(), expected_actions2);

            operation::apply_writes_to_state(&mut state, output1.writes().iter().cloned());
            operation::apply_writes_to_state(&mut state, output2.writes().iter().cloned());

            assert!(state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), height + 1);
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[0..idx + 1]
            );
        }
    }

    #[test]
    fn test_l1_reorg() {
        let database = get_common_db();
        let params = gen_params();
        let mut state = gen_client_state(Some(&params));

        let height = params.rollup().genesis_l1_height;
        let event = SyncEvent::L1Revert(height);

        let l1_block: L1BlockManifest = ArbitraryGenerator::new().generate();
        database
            .l1_store()
            .put_block_data(height, l1_block.clone(), vec![])
            .unwrap();

        let res = process_event(&state, &event, database.as_ref(), &params).unwrap();
        eprintln!("process_event on {event:?} -> {res:?}");
        let expected_writes = [ClientStateWrite::RollbackL1BlocksTo(height)];
        let expected_actions = [];

        assert_eq!(res.actions(), expected_actions);
        assert_eq!(res.writes(), expected_writes);
    }
}
