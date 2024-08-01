//! Core state transition function.
#![allow(unused)] // still under development

use tracing::*;

use alpen_express_db::traits::{Database, L1DataProvider, L2DataProvider};
use alpen_express_primitives::prelude::*;
use alpen_express_state::client_state::*;
use alpen_express_state::operation::*;
use alpen_express_state::sync_event::SyncEvent;

use crate::errors::*;

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
            // FIXME this doesn't do any SPV checks to make sure we only go to
            // a longer chain, it just does it unconditionally
            let l1prov = database.l1_provider();
            let _blkmf = l1prov.get_block_manifest(*height)?;

            // TODO do the consensus checks

            writes.push(ClientStateWrite::AcceptL1Block(*l1blkid));

            // If we have some number of L1 blocks finalized, also emit an `UpdateBuried` write
            if *height >= params.rollup().l1_reorg_safe_depth as u64 + state.buried_l1_height() {
                writes.push(ClientStateWrite::UpdateBuried(state.buried_l1_height() + 1));
            }

            let l1v = state.l1_view();

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
                    writes.push(ClientStateWrite::ActivateChain);
                }
            }
        }

        SyncEvent::L1Revert(to_height) => {
            let l1prov = database.l1_provider();
            let blkmf = l1prov
                .get_block_manifest(*to_height)?
                .ok_or(Error::MissingL1BlockHeight(*to_height))?;
            let blkid = blkmf.block_hash().into();
            writes.push(ClientStateWrite::RollbackL1BlocksTo(blkid));
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
                actions.push(SyncAction::FinalizeBlock(*last))
            } else {
                // TODO we can expand this later to make more sense
                return Err(Error::MissingClientSyncState);
            }
        }

        SyncEvent::ComputedGenesis(gblkid) => {
            // Just construct the sync state for the genesis.
            let ss = SyncState::from_genesis_blkid(*gblkid);
            writes.push(ClientStateWrite::ReplaceSync(Box::new(ss)));
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
    use alpen_express_db::{client_state, traits::L1DataStore};
    use alpen_express_primitives::{block_credential, l1::L1BlockManifest};
    use alpen_express_state::{l1::L1BlockId, operation};
    use alpen_test_utils::{get_common_db, get_rocksdb_tmp_instance, ArbitraryGenerator};

    use crate::genesis;

    use super::*;

    fn get_params() -> Params {
        Params {
            rollup: RollupParams {
                block_time: 1000,
                cred_rule: block_credential::CredRule::Unchecked,
                horizon_l1_height: 3,
                genesis_l1_height: 5,
                evm_genesis_block_hash: Buf32(
                    "0x37ad61cff1367467a98cf7c54c4ac99e989f1fbb1bc1e646235e90c065c565ba"
                        .parse()
                        .unwrap(),
                ),
                evm_genesis_block_state_root: Buf32(
                    "0x351714af72d74259f45cd7eab0b04527cd40e74836a45abcae50f92d919d988f"
                        .parse()
                        .unwrap(),
                ),
                l1_reorg_safe_depth: 5,
            },
            run: RunParams {
                l1_follow_distance: 3,
            },
        }
    }

    fn gen_client_state(params: &Params) -> ClientState {
        ClientState::from_genesis_params(
            params.rollup.genesis_l1_height,
            params.rollup.genesis_l1_height,
        )
    }

    #[test]
    fn handle_l1_block() {
        let database = get_common_db();
        let params = get_params();
        let mut state = gen_client_state(&params);
        let l1_block_id = L1BlockId::from(Buf32::default());

        // before the genesis of L2 is reached
        {
            let event = SyncEvent::L1Block(1, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let writes = output.writes();
            let actions = output.actions();

            let expection_writes = [ClientStateWrite::AcceptL1Block(l1_block_id)];
            let expected_actions = [];

            assert_eq!(writes, expection_writes);
            assert_eq!(actions, expected_actions);

            operation::apply_writes_to_state(&mut state, writes.iter().cloned());
        }

        // after the genesis of L2 is reached
        {
            let height = params.rollup.genesis_l1_height + 3;
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let writes = output.writes();
            let actions = output.actions();

            let expection_writes = [
                ClientStateWrite::AcceptL1Block(l1_block_id),
                ClientStateWrite::ActivateChain,
            ];
            let expected_actions = [];

            assert_eq!(writes, expection_writes);
            assert_eq!(actions, expected_actions);

            operation::apply_writes_to_state(&mut state, writes.iter().cloned());
        }

        // after l1_reorg_depth is reached
        {
            let height = params.rollup.genesis_l1_height + params.rollup.l1_reorg_safe_depth as u64;
            let event = SyncEvent::L1Block(height, l1_block_id);

            let output = process_event(&state, &event, database.as_ref(), &params).unwrap();

            let expection_writes = [
                ClientStateWrite::AcceptL1Block(l1_block_id),
                ClientStateWrite::UpdateBuried(params.rollup.genesis_l1_height + 1),
            ];
            let expected_actions = [];

            assert_eq!(output.writes(), expection_writes);
            assert_eq!(output.actions(), expected_actions);

            operation::apply_writes_to_state(&mut state, output.writes().iter().cloned());
        }
    }

    #[test]
    fn handle_l1_revert() {
        let database = get_common_db();
        let params = get_params();
        let mut state = gen_client_state(&params);

        let height = 5;
        let event = SyncEvent::L1Revert(height);

        let output = process_event(&state, &event, database.as_ref(), &params);
        assert!(output.is_err_and(|x| matches!(x, Error::MissingL1BlockHeight(height))));

        let l1_block: L1BlockManifest = ArbitraryGenerator::new().generate();
        database
            .l1_store()
            .put_block_data(height, l1_block.clone(), vec![])
            .unwrap();

        let output = process_event(&state, &event, database.as_ref(), &params).unwrap();
        let expectation_writes = [ClientStateWrite::RollbackL1BlocksTo(
            l1_block.block_hash().into(),
        )];
        let expected_actions = [];

        assert_eq!(output.actions(), expected_actions);
        assert_eq!(output.writes(), expectation_writes);
    }
}
