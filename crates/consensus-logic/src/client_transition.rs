//! Core state transition function.
#![allow(unused)] // still under development

use alpen_express_db::traits::{Database, L1DataProvider, L2DataProvider, L2DataStore};
use alpen_express_primitives::prelude::*;
use alpen_express_state::{client_state::*, header::L2Header, operation::*, sync_event::SyncEvent};
use tracing::*;

use crate::{errors::*, genesis::make_genesis_block};

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
            let _new_block_mf = l1prov.get_block_manifest(*height)?;

            let l1v = state.l1_view();

            // TODO do the consensus checks

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
            // TODO clean up this bookkeeping slightly
            let keep_window = params.rollup().l1_reorg_safe_depth as u64;
            let maturable_height = next_exp_height.saturating_sub(keep_window);
            if maturable_height > state.next_exp_l1_block() {
                writes.push(ClientStateWrite::UpdateBuried(maturable_height));
            }

            if let Some(ss) = state.sync() {
                // TODO figure out what to do here
            } else {
                let horizon_ht = params.rollup.horizon_l1_height;
                let genesis_ht = params.rollup.genesis_l1_height;

                // TODO make params configurable
                let genesis_threshold = genesis_ht + 3;

                // If necessary, activeate the chain!
                if !state.is_chain_active() && *height >= genesis_threshold {
                    debug!("emitting chain activation");
                    let genesis_block = make_genesis_block(params);

                    writes.push(ClientStateWrite::ActivateChain);
                    writes.push(ClientStateWrite::ReplaceSync(Box::new(
                        SyncState::from_genesis_blkid(genesis_block.header().get_blockid()),
                    )));
                    actions.push(SyncAction::L2Genesis(*l1blkid));
                }
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

        SyncEvent::L1DABatch(blkids) => {
            if blkids.is_empty() {
                warn!("empty L1DABatch");
            }

            if let Some(ss) = state.sync() {
                // TODO load it up and figure out what's there, see if we have to
                // load the state updates from L1 or something
                let l2prov = database.l2_provider();

                for id in blkids {
                    let _block = l2prov
                        .get_block_data(*id)?
                        .ok_or(Error::MissingL2Block(*id))?;

                    // TODO do whatever changes we have to to accept the new block
                }

                let last = blkids.last().unwrap();
                writes.push(ClientStateWrite::UpdateFinalized(*last));
                actions.push(SyncAction::FinalizeBlock(*last));
            } else {
                // TODO we can expand this later to make more sense
                return Err(Error::MissingClientSyncState);
            }
        }

        SyncEvent::NewTipBlock(blkid) => {
            let l2prov = database.l2_provider();
            let _block = l2prov
                .get_block_data(*blkid)?
                .ok_or(Error::MissingL2Block(*blkid))?;

            // TODO better checks here
            writes.push(ClientStateWrite::AcceptL2Block(*blkid));
            actions.push(SyncAction::UpdateTip(*blkid));
        }
    }

    Ok(ClientUpdateOutput::new(writes, actions))
}

#[cfg(test)]
mod tests {
    use alpen_express_db::traits::L1DataStore;
    use alpen_express_primitives::{block_credential, l1::L1BlockManifest};
    use alpen_express_rocksdb::test_utils::get_common_db;
    use alpen_express_state::{l1::L1BlockId, operation};
    use alpen_test_utils::{
        bitcoin::gen_l1_chain,
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
        let l1_chain = gen_l1_chain(15);

        for (i, b) in l1_chain.iter().enumerate() {
            let l1_store = database.l1_store();
            l1_store
                .put_block_data(i as u64, b.clone(), Vec::new())
                .expect("test: insert blocks");
        }

        let blkids: Vec<L1BlockId> = l1_chain.iter().map(|b| b.block_hash().into()).collect();
        let horizon = params.rollup().horizon_l1_height;
        let genesis = params.rollup().genesis_l1_height;

        // at horizon block
        {
            let height = horizon;
            let l1_block_id = l1_chain[height as usize].block_hash().into();
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
                &blkids[height as usize..height as usize + 1]
            );
        }

        // at horizon block + 1
        {
            let height = params.rollup().horizon_l1_height + 1;
            let l1_block_id = l1_chain[height as usize].block_hash().into();
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
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[horizon as usize..horizon as usize + 2]
            );
        }

        // as the genesis of L2 is reached, but not locked in yet
        {
            let height = genesis;
            let l1_block_id = l1_chain[height as usize].block_hash().into();
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
                &blkids[horizon as usize..height as usize + 1]
            );
        }

        // genesis + 1
        {
            let height = genesis + 1;
            let l1_block_id = l1_chain[height as usize].block_hash().into();
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
                &blkids[horizon as usize..height as usize + 1]
            );
        }

        // genesis + 2
        {
            let height = genesis + 2;
            let l1_block_id = l1_chain[height as usize].block_hash().into();
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
                &blkids[horizon as usize..height as usize + 1]
            );
        }

        // genesis + 3, where we should lock in genesis
        {
            let height = genesis + 3;
            let l1_block_id = l1_chain[height as usize].block_hash().into();
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let genesis_block = genesis::make_genesis_block(&params);
            let genesis_blockid = genesis_block.header().get_blockid();

            let expected_writes = [
                ClientStateWrite::AcceptL1Block(l1_block_id),
                ClientStateWrite::ActivateChain,
                ClientStateWrite::ReplaceSync(Box::new(SyncState::from_genesis_blkid(
                    genesis_blockid,
                ))),
            ];
            let expected_actions = [SyncAction::L2Genesis(l1_block_id)];

            assert_eq!(output.writes(), expected_writes);
            assert_eq!(output.actions(), expected_actions);

            operation::apply_writes_to_state(&mut state, output.writes().iter().cloned());

            assert!(state.is_chain_active());
            assert_eq!(state.most_recent_l1_block(), Some(&l1_block_id));
            assert_eq!(state.next_exp_l1_block(), height + 1);
            assert_eq!(
                state.l1_view().local_unaccepted_blocks(),
                &blkids[horizon as usize..height as usize + 1]
            );
        }
    }

    #[test]
    fn test_l1_reorg() {
        let database = get_common_db();
        let params = gen_params();
        let mut state = gen_client_state(Some(&params));

        let height = 5;
        let event = SyncEvent::L1Revert(height);

        let l1_block: L1BlockManifest = ArbitraryGenerator::new().generate();
        database
            .l1_store()
            .put_block_data(height, l1_block.clone(), vec![])
            .unwrap();

        let res = process_event(&state, &event, database.as_ref(), &params).unwrap();
        eprintln!("process_event on {event:?} -> {res:?}");
        let expected_writes = [ClientStateWrite::RollbackL1BlocksTo(5)];
        let expected_actions = [];

        assert_eq!(res.actions(), expected_actions);
        assert_eq!(res.writes(), expected_writes);
    }
}
