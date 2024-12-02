//! Core state transition function.
#![allow(unused)] // still under development

use std::cmp::min;

use bitcoin::block::Header;
use strata_db::traits::{ChainstateDatabase, Database, L1Database, L2BlockDatabase};
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
            let l1_db = database.l1_db();
            let block_mf = l1_db
                .get_block_manifest(*height)?
                .ok_or(Error::MissingL1BlockHeight(*height))?;

            let l1v = state.l1_view();
            let l1_vs = state.l1_view().tip_verification_state();

            // Do the consensus checks
            if let Some(l1_vs) = l1_vs {
                let l1_vs_height = l1_vs.last_verified_block_num as u64;
                let mut updated_l1vs = l1_vs.clone();
                if l1_vs_height < l1v.tip_height() {
                    for height in (l1_vs_height..l1v.tip_height()) {
                        let block_mf = l1_db
                            .get_block_manifest(height)?
                            .ok_or(Error::MissingL1BlockHeight(height))?;
                        let header: Header =
                            bitcoin::consensus::deserialize(block_mf.header()).unwrap();
                        updated_l1vs = updated_l1vs
                            .check_and_update_continuity_new(&header, &get_btc_params());
                    }
                }
                writes.push(ClientStateWrite::UpdateVerificationState(updated_l1vs))
            }

            // Only accept the block if it's the next block in the chain we expect to accept.
            let cur_seen_tip_height = l1v.tip_height();
            let next_exp_height = l1v.next_expected_block();
            if next_exp_height > params.rollup().horizon_l1_height {
                // TODO check that the new block we're trying to add has the same parent as the tip
                // block
                let cur_tip_block = l1_db
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
                let (wrs, acts) = handle_maturable_height(maturable_height, state);
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
            let l1_db = database.l1_db();

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
                let l2_db = database.l2_db();

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
            let l2_db = database.l2_db();
            let block = l2_db
                .get_block_data(*blkid)?
                .ok_or(Error::MissingL2Block(*blkid))?;

            // TODO: get chainstate idx from blkid OR pass correct idx in sync event
            let block_idx = block.header().blockidx();
            let chainstate_db = database.chain_state_db();
            let chainstate = chainstate_db
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
        }
    }

    Ok(ClientUpdateOutput::new(writes, actions))
}

fn handle_maturable_height(
    maturable_height: u64,
    state: &ClientState,
) -> (Vec<ClientStateWrite>, Vec<SyncAction>) {
    let mut writes = Vec::new();
    let mut actions = Vec::new();

    // If there are checkpoints at or before the maturable height, mark them as finalized
    if state
        .l1_view()
        .has_verified_checkpoint_before(maturable_height)
    {
        debug!(%maturable_height, "Writing CheckpointFinalized");
        writes.push(ClientStateWrite::CheckpointFinalized(maturable_height));

        // Emit sync action for finalizing a l2 block
        if let Some(checkpt) = state
            .l1_view()
            .get_last_verified_checkpoint_before(maturable_height)
        {
            actions.push(SyncAction::FinalizeBlock(checkpt.batch_info.l2_blockid));
        } else {
            warn!(
            %maturable_height,
            "expected to find blockid corresponding to buried l1 height in confirmed_blocks but could not find"
            );
        }
    }
    (writes, actions)
}

/// Filters a list of `BatchCheckpoint`s, returning only those that form a valid sequence
/// of checkpoints.
///
/// A valid checkpoint is one whose proof passes verification, and its index follows
/// sequentially from the previous valid checkpoint.
///
/// # Arguments
///
/// * `state` - The client's current state, which provides the L1 view and pending checkpoints.
/// * `checkpoints` - A slice of `BatchCheckpoint`s to be filtered.
/// * `params` - Parameters required for verifying checkpoint proofs.
///
/// # Returns
///
/// A vector containing the valid sequence of `BatchCheckpoint`s, starting from the first valid one.
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
    use strata_db::traits::L1Database;
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

    struct TestEvent {
        event: SyncEvent,
        expected_writes: Vec<ClientStateWrite>,
        expected_actions: Vec<SyncAction>,
    }

    struct TestCase {
        description: &'static str,
        events: Vec<TestEvent>, // List of events to process
        state_assertions: Box<dyn Fn(&ClientState)>, // Closure to verify state after all events
    }

    fn run_test_cases<D: Database>(
        test_cases: Vec<TestCase>,
        state: &mut ClientState,
        database: &D,
        params: &Params,
    ) {
        for case in test_cases {
            println!("Running test case: {}", case.description);
            let mut outputs = Vec::new();
            for (i, test_event) in case.events.iter().enumerate() {
                let output = process_event(state, &test_event.event, database, params).unwrap();
                outputs.push(output.clone());
                assert_eq!(
                    output.writes(),
                    test_event.expected_writes,
                    "Failed on writes for event {} in test case: {}",
                    i + 1,
                    case.description
                );
                assert_eq!(
                    output.actions(),
                    test_event.expected_actions,
                    "Failed on actions for event {} in test case: {}",
                    i + 1,
                    case.description
                );
            }

            for output in outputs {
                operation::apply_writes_to_state(state, output.writes().iter().cloned());
            }

            // Run the state assertions after all events
            (case.state_assertions)(state);
        }
    }

    #[test]
    fn test_genesis() {
        let database = get_common_db();
        let params = gen_params();
        let mut state = gen_client_state(Some(&params));

        let horizon = params.rollup().horizon_l1_height;
        let genesis = params.rollup().genesis_l1_height;

        let chain = get_btc_chain();
        let l1_chain = chain.get_block_manifests(horizon as u32, 10);
        let l1_verification_state =
            chain.get_verification_state(genesis as u32 + 1, &MAINNET.clone().into());

        let genesis_block = genesis::make_genesis_block(&params);
        let genesis_blockid = genesis_block.header().get_blockid();

        let l1_db = database.l1_db();
        for (i, b) in l1_chain.iter().enumerate() {
            l1_db
                .put_block_data(i as u64 + horizon, b.clone(), Vec::new())
                .expect("test: insert blocks");
        }
        let blkids: Vec<L1BlockId> = l1_chain.iter().map(|b| b.block_hash().into()).collect();

        let test_cases = vec![
            TestCase {
                description: "At horizon block",
                events: vec![TestEvent {
                    event: SyncEvent::L1Block(horizon, l1_chain[0].block_hash().into()),
                    expected_writes: vec![ClientStateWrite::AcceptL1Block(
                        l1_chain[0].block_hash().into(),
                    )],
                    expected_actions: vec![],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[0].block_hash().into())
                        );
                        assert_eq!(state.next_exp_l1_block(), horizon + 1);
                    }
                }),
            },
            TestCase {
                description: "At horizon block + 1",
                events: vec![TestEvent {
                    event: SyncEvent::L1Block(horizon + 1, l1_chain[1].block_hash().into()),
                    expected_writes: vec![ClientStateWrite::AcceptL1Block(
                        l1_chain[1].block_hash().into(),
                    )],
                    expected_actions: vec![],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[1].block_hash().into())
                        );
                        // Because values for horizon is 40318, genesis is 40320
                        assert_eq!(state.next_exp_l1_block(), genesis);
                    }
                }),
            },
            TestCase {
                description: "As the genesis of L2 is reached but not locked in yet",
                events: vec![TestEvent {
                    event: SyncEvent::L1Block(
                        genesis,
                        l1_chain[(genesis - horizon) as usize].block_hash().into(),
                    ),
                    expected_writes: vec![ClientStateWrite::AcceptL1Block(
                        l1_chain[(genesis - horizon) as usize].block_hash().into(),
                    )],
                    expected_actions: vec![],
                }],
                state_assertions: Box::new(move |state| {
                    assert!(!state.is_chain_active());
                    assert_eq!(state.next_exp_l1_block(), genesis + 1);
                }),
            },
            TestCase {
                description: "At genesis + 1",
                events: vec![TestEvent {
                    event: SyncEvent::L1Block(
                        genesis + 1,
                        l1_chain[(genesis + 1 - horizon) as usize]
                            .block_hash()
                            .into(),
                    ),
                    expected_writes: vec![ClientStateWrite::AcceptL1Block(
                        l1_chain[(genesis + 1 - horizon) as usize]
                            .block_hash()
                            .into(),
                    )],
                    expected_actions: vec![],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    let blkids = blkids.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(
                                &l1_chain[(genesis + 1 - horizon) as usize]
                                    .block_hash()
                                    .into()
                            )
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
                events: vec![TestEvent {
                    event: SyncEvent::L1Block(
                        genesis + 2,
                        l1_chain[(genesis + 2 - horizon) as usize]
                            .block_hash()
                            .into(),
                    ),
                    expected_writes: vec![ClientStateWrite::AcceptL1Block(
                        l1_chain[(genesis + 2 - horizon) as usize]
                            .block_hash()
                            .into(),
                    )],
                    expected_actions: vec![],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    let blkids = blkids.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(
                                &l1_chain[(genesis + 2 - horizon) as usize]
                                    .block_hash()
                                    .into()
                            )
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
                events: vec![
                    TestEvent {
                        event: SyncEvent::L1BlockGenesis(
                            genesis + 3,
                            l1_verification_state.clone(),
                        ),
                        expected_writes: vec![
                            ClientStateWrite::ActivateChain,
                            ClientStateWrite::UpdateVerificationState(
                                l1_verification_state.clone(),
                            ),
                            ClientStateWrite::ReplaceSync(Box::new(SyncState::from_genesis_blkid(
                                genesis_blockid,
                            ))),
                        ],
                        expected_actions: vec![SyncAction::L2Genesis(
                            l1_chain[(genesis - horizon) as usize].block_hash().into(),
                        )],
                    },
                    TestEvent {
                        event: SyncEvent::L1Block(
                            genesis + 3,
                            l1_chain[(genesis + 3 - horizon) as usize]
                                .block_hash()
                                .into(),
                        ),
                        expected_writes: vec![ClientStateWrite::AcceptL1Block(
                            l1_chain[(genesis + 3 - horizon) as usize]
                                .block_hash()
                                .into(),
                        )],
                        expected_actions: vec![],
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
        ];

        run_test_cases(test_cases, &mut state, database.as_ref(), &params);
    }

    // TODO: so after reverting, should we not apply writes ???
    // Keeping it out of our generalized table approach as this is different
    #[test]
    fn test_l1_reorg() {
        let database = get_common_db();
        let params = gen_params();
        let mut state = gen_client_state(Some(&params));

        let height = params.rollup().genesis_l1_height;
        let event = SyncEvent::L1Revert(height);

        let l1_block: L1BlockManifest = ArbitraryGenerator::new().generate();
        database
            .l1_db()
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
