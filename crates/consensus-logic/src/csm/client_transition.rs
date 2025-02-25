//! Core state transition function.
#![allow(unused)] // still under development

use std::cmp::min;

use bitcoin::block::Header;
use strata_db::traits::{ChainstateDatabase, Database, L1Database, L2BlockDatabase};
use strata_primitives::{
    batch::{verify_signed_checkpoint_sig, BatchInfo, Checkpoint},
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

use crate::{checkpoint_verification::verify_checkpoint, errors::*, genesis::make_genesis_block};

/// Interface for external context necessary specifically for event validation.
pub trait EventContext {
    fn get_l1_block_manifest(&self, blockid: &L1BlockId) -> Result<L1BlockManifest, Error>;
    fn get_l1_block_manifest_at_height(&self, height: u64) -> Result<L1BlockManifest, Error>;
    fn get_l2_block_data(&self, blockid: &L2BlockId) -> Result<L2BlockBundle, Error>;
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
    fn get_l1_block_manifest(&self, blockid: &L1BlockId) -> Result<L1BlockManifest, Error> {
        self.storage
            .l1()
            .get_block_manifest(blockid)?
            .ok_or(Error::MissingL1Block(*blockid))
    }
    fn get_l1_block_manifest_at_height(&self, height: u64) -> Result<L1BlockManifest, Error> {
        self.storage
            .l1()
            .get_block_manifest_at_height(height)?
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
            let genesis_trigger = params.rollup().genesis_l1_height;
            if height < genesis_trigger {
                #[cfg(test)]
                eprintln!(
                    "early L1 block at h={height} (gt={genesis_trigger}) you may have set up the test env wrong"
                );

                warn!(%height, "ignoring unexpected L1Block event before horizon");
                return Ok(());
            }

            // This doesn't do any SPV checks to make sure we only go to a
            // a longer chain, it just does it unconditionally.  This is fine,
            // since we'll be refactoring this more deeply soonish.
            let block_mf = context.get_l1_block_manifest(block.blkid())?;
            handle_block(state, block, &block_mf, context, params)?;
            Ok(())
        }

        SyncEvent::L1Revert(block) => {
            // TODO move this logic out into this function
            state.rollback_l1_blocks(*block);
            Ok(())
        }
    }
}

fn handle_block(
    state: &mut ClientStateMut,
    block: &L1BlockCommitment,
    block_mf: &L1BlockManifest,
    context: &impl EventContext,
    params: &Params,
) -> Result<(), Error> {
    let height = block.height();
    let l1blkid = block.blkid();

    let next_exp_height = state.state().next_exp_l1_block();
    let old_final_epoch = state.state().get_declared_final_epoch().copied();

    // We probably should have gotten the L1Genesis message by now but
    // let's just do this anyways.
    if height == params.rollup().genesis_l1_height {
        // Do genesis here.
        let istate = process_genesis_trigger_block(block_mf, params.rollup())?;
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

        let new_istate = process_l1_block(prev_istate, height, block_mf, params.rollup())?;
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

fn process_genesis_trigger_block(
    block_mf: &L1BlockManifest,
    params: &RollupParams,
) -> Result<InternalState, Error> {
    // TODO maybe more bookkeeping?
    Ok(InternalState::new(*block_mf.blkid(), None))
}

fn process_l1_block(
    state: &InternalState,
    height: u64,
    block_mf: &L1BlockManifest,
    params: &RollupParams,
) -> Result<InternalState, Error> {
    let blkid = block_mf.blkid();
    let mut checkpoint = state.last_checkpoint().cloned();

    // Iterate through all of the protocol operations in all of the txs.
    // TODO split out each proto op handling into a separate function
    for tx in block_mf.txs() {
        for op in tx.protocol_ops() {
            match op {
                ProtocolOperation::Checkpoint(signed_ckpt) => {
                    // Before we do anything, check its signature.
                    if !verify_signed_checkpoint_sig(signed_ckpt, &params.cred_rule) {
                        warn!(%height, "ignoring checkpointing with invalid signature");
                        continue;
                    }

                    let ckpt = signed_ckpt.checkpoint();

                    // Now do the more thorough checks
                    if verify_checkpoint(ckpt, checkpoint.as_ref(), params).is_err() {
                        // If it's invalid then just print a warning and move on.
                        warn!(%height, "ignoring invalid checkpoint in L1 block");
                        continue;
                    }

                    // Construct the state bookkeeping entry for the checkpoint.
                    let l1ckpt = L1Checkpoint::new(
                        ckpt.batch_info().clone(),
                        ckpt.batch_transition().clone(),
                        ckpt.base_state_commitment().clone(),
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

    Ok(InternalState::new(*blkid, checkpoint))
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
                return Err(e);
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

#[cfg(test)]
mod tests {
    use bitcoin::{params::MAINNET, BlockHash};
    use strata_db::traits::L1Database;
    use strata_primitives::{
        block_credential,
        l1::{L1BlockManifest, L1HeaderRecord},
    };
    use strata_rocksdb::test_utils::get_common_db;
    use strata_state::{l1::L1BlockId, operation};
    use strata_test_utils::{
        bitcoin::gen_l1_chain,
        bitcoin_mainnet_segment::BtcChainSegment,
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
                chainseg: BtcChainSegment::load(),
            }
        }
    }

    impl EventContext for DummyEventContext {
        fn get_l1_block_manifest(&self, blockid: &L1BlockId) -> Result<L1BlockManifest, Error> {
            let blockhash: BlockHash = (*blockid).into();
            Ok(self
                .chainseg
                .get_block_manifest_by_blockhash(&blockhash)
                .unwrap())
        }

        fn get_l1_block_manifest_at_height(&self, height: u64) -> Result<L1BlockManifest, Error> {
            let rec = self.chainseg.get_header_record(height).unwrap();
            Ok(L1BlockManifest::new(
                rec,
                HeaderVerificationState::default(),
                Vec::new(),
                0,
                height,
            ))
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
                let event = &test_event.event;
                eprintln!("giving sync event {event}");
                process_event(&mut state_mut, event, &context, params).unwrap();
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

        let horizon = params.rollup().horizon_l1_height as u64;
        let genesis = params.rollup().genesis_l1_height as u64;
        let reorg_safe_depth = params.rollup().l1_reorg_safe_depth;

        let chain = BtcChainSegment::load();
        let l1_verification_state = chain
            .get_verification_state(genesis + 1, reorg_safe_depth)
            .unwrap();

        let genesis_block = genesis::make_genesis_block(&params);
        let genesis_blockid = genesis_block.header().get_blockid();
        let l1_chain = chain.get_header_records(horizon, 10).unwrap();

        let l1_blocks = l1_chain
            .iter()
            .enumerate()
            .map(|(i, block)| L1BlockCommitment::new(horizon + i as u64, *block.blkid()))
            .collect::<Vec<_>>();

        let blkids: Vec<L1BlockId> = l1_chain.iter().map(|b| *b.blkid()).collect();

        let test_cases = [
            // These are kinda weird out because we got rid of pre-genesis
            // tracking and just discard these L1 blocks that are before
            // genesis.  We might re-add this later if the project demands it.
            TestCase {
                description: "At horizon block",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(l1_blocks[0]),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                    }
                }),
            },
            TestCase {
                description: "At horizon block + 1",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(l1_blocks[1]),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    move |state| {
                        assert!(!state.is_chain_active());
                        /*assert_eq!(
                            state.most_recent_l1_block(),
                            Some(&l1_chain[1].blkid())
                        );*/
                        // Because values for horizon is 40318, genesis is 40320
                        assert_eq!(state.next_exp_l1_block(), genesis);
                    }
                }),
            },
            TestCase {
                // We're assuming no rollback here.
                description: "At L2 genesis trigger L1 block reached we lock in",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(l1_blocks[2]),
                    expected_actions: &[SyncAction::L2Genesis(*l1_blocks[2].blkid())],
                }],
                state_assertions: Box::new(move |state| {
                    assert!(state.is_chain_active());
                    assert_eq!(state.next_exp_l1_block(), genesis + 1);
                }),
            },
            TestCase {
                description: "At genesis + 1",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(l1_blocks[3]),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    let blkids = blkids.clone();
                    move |state| {
                        assert!(state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(l1_chain[(genesis + 1 - horizon) as usize].blkid(),)
                        );
                        assert_eq!(state.next_exp_l1_block(), genesis + 2);
                    }
                }),
            },
            TestCase {
                description: "At genesis + 2",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(l1_blocks[4]),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({
                    let l1_chain = l1_chain.clone();
                    let blkids = blkids.clone();
                    move |state| {
                        assert!(state.is_chain_active());
                        assert_eq!(
                            state.most_recent_l1_block(),
                            Some(l1_chain[(genesis + 2 - horizon) as usize].blkid())
                        );
                        assert_eq!(state.next_exp_l1_block(), genesis + 3);
                    }
                }),
            },
            TestCase {
                description: "At genesis + 3, lock in genesis",
                events: &[TestEvent {
                    event: SyncEvent::L1Block(l1_blocks[5]),
                    expected_actions: &[],
                }],
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
                    event: SyncEvent::L1Revert(l1_blocks[4]),
                    expected_actions: &[],
                }],
                state_assertions: Box::new({ move |state| {} }),
            },
        ];

        run_test_cases(&test_cases, &mut state, &params);
    }
}
