use alpen_express_db::traits::{ChainstateProvider, Database, L2DataProvider};
use alpen_express_primitives::params::Params;
use alpen_express_state::{client_state::ClientState, header::L2Header, id::L2BlockId};

use super::types::{BatchCommitmentDuty, BlockSigningDuty, Duty, Identity};
use crate::errors::Error;

/// Extracts new duties given a consensus state and a identity.
pub fn extract_duties<D: Database>(
    state: &ClientState,
    _ident: &Identity,
    database: &D,
    params: &Params,
) -> Result<Vec<Duty>, Error> {
    // If a sync state isn't present then we probably don't have anything we
    // want to do.  We might change this later.
    let Some(ss) = state.sync() else {
        return Ok(Vec::new());
    };

    let tip_blkid = *ss.chain_tip_blkid();

    // Figure out the block slot from the tip blockid.
    // TODO include the block slot in the consensus state
    let l2prov = database.l2_provider();
    let block = l2prov
        .get_block_data(tip_blkid)?
        .ok_or(Error::MissingL2Block(tip_blkid))?;
    let block_idx = block.header().blockidx();

    // Since we're not rotating sequencers, for now we just *always* produce a
    // new block.
    let duty_data = BlockSigningDuty::new_simple(block_idx + 1, tip_blkid);
    let mut duties = vec![Duty::SignBlock(duty_data)];

    duties.append(&mut extract_batch_duties(state, _ident, database, params)?);

    Ok(duties)
}

fn extract_batch_duties<D: Database>(
    state: &ClientState,
    _ident: &Identity,
    database: &D,
    params: &Params,
) -> Result<Vec<Duty>, Error> {
    // If a sync state isn't present then we probably don't have anything we
    // want to do.  We might change this later.
    let Some(ss) = state.sync() else {
        return Ok(Vec::new());
    };

    let finalized_blkid = *ss.finalized_blkid();

    // Generate all valid batches from last finalized block till latest.
    // Deduplication is managed by duty executor and l1 writer
    let duties = generate_batches(finalized_blkid, database, params)?
        .into_iter()
        .map(|(slot, blockid)| Duty::CommitBatch(BatchCommitmentDuty::new(slot, blockid)))
        .collect();

    Ok(duties)
}

// NOTE: generated batches MUST be deterministic for a given finalized_blkid and set of l2 blocks
fn generate_batches<D: Database>(
    finalized_blkid: L2BlockId,
    database: &D,
    params: &Params,
) -> Result<Vec<(u64, L2BlockId)>, Error> {
    let chainstate_provider = database.chainstate_provider();
    let l2prov = database.l2_provider();

    // should a safety factor be subtracted from this?
    let tip_blockidx = chainstate_provider.get_last_state_idx()?;

    let finalized_block = l2prov
        .get_block_data(finalized_blkid)?
        .ok_or(Error::MissingL2Block(finalized_blkid))?;

    // L2 Block finalization happens at batch level.
    // So finalized block idx is idx of last block of latest finalized batch.
    let last_idx = finalized_block.header().blockidx();

    let mut batches = Vec::new();

    let mut next_idx = last_idx + params.rollup.target_l2_batch_size;
    while next_idx <= tip_blockidx {
        let chain_state = chainstate_provider
            .get_toplevel_state(next_idx)?
            .ok_or(Error::MissingCheckpoint(next_idx))?;

        batches.push((next_idx, chain_state.chain_tip_blockid()));

        // probably more sophisticated way to delimit batches here
        next_idx += params.rollup.target_l2_batch_size;
    }

    Ok(batches)
}

#[cfg(test)]
mod tests {

    use alpen_express_db::traits::{ChainstateStore, L2DataStore};
    use alpen_express_primitives::{
        block_credential,
        buf::Buf32,
        params::{RollupParams, RunParams},
        utils::get_test_schnorr_keys,
    };
    use alpen_express_rocksdb::test_utils::get_common_db;
    use alpen_express_state::{
        block::{L2Block, L2BlockBody, L2BlockBundle},
        bridge_state::OperatorTable,
        chain_state::{ChainState, GenesisStateConfig},
        exec_env::ExecEnvState,
        exec_update::UpdateInput,
        header::{L2BlockHeader, SignedL2BlockHeader},
        l1::{L1HeaderRecord, L1ViewState},
        state_op::{StateOp, WriteBatch},
    };

    use super::*;

    pub fn gen_params() -> Params {
        Params {
            rollup: RollupParams {
                rollup_name: "express".to_string(),
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
                batch_l2_blocks_target: 64,
                operator_signing_keys: Vec::from(get_test_schnorr_keys()),
                target_l2_batch_size: 64,
            },
            run: RunParams {
                l2_blocks_fetch_limit: 1000,
                l1_follow_distance: 3,
                client_checkpoint_interval: 10,
            },
        }
    }

    fn insert_blocks_chainstate(
        block_count: u64,
        evm_genesis_state_root: Buf32,
        database: &impl Database,
    ) -> Vec<L2BlockId> {
        let arb = alpen_test_utils::ArbitraryGenerator::new();

        let blocks: Vec<_> = (0..block_count)
            .map(|idx| {
                let block_body: L2BlockBody = arb.generate();
                let block_header: L2BlockHeader = L2BlockHeader::new(
                    idx,
                    arb.generate(),
                    arb.generate(),
                    &block_body,
                    arb.generate(),
                );
                L2BlockBundle::new(
                    L2Block::new(
                        SignedL2BlockHeader::new(block_header, arb.generate()),
                        arb.generate(),
                    ),
                    arb.generate(),
                )
            })
            .collect();
        let blockids: Vec<L2BlockId> = blocks
            .iter()
            .map(|block| block.header().get_blockid())
            .collect();

        for block in blocks {
            database.l2_store().put_block_data(block.clone()).unwrap();
        }

        let exec_env_state = ExecEnvState::from_base_input(
            UpdateInput::new(0, Buf32::zero(), Vec::new()),
            evm_genesis_state_root,
        );

        let arb_header: [u8; 80] = arb.generate();
        let genesis_chain_state = ChainState::from_genesis(
            GenesisStateConfig::new(OperatorTable::new_empty()),
            blockids[0],
            L1ViewState::new_at_genesis(
                3,
                5,
                L1HeaderRecord::new(arb_header.to_vec(), Buf32::zero()),
            ),
            exec_env_state,
        );

        database
            .chainstate_store()
            .write_genesis_state(&genesis_chain_state)
            .unwrap();

        for idx in 1..block_count {
            let batch = WriteBatch::new(vec![StateOp::SetSlotAndTipBlock(
                idx,
                blockids[idx as usize],
            )]);
            database
                .chainstate_store()
                .write_state_update(idx, &batch)
                .unwrap();
        }

        blockids
    }

    #[test]
    fn test_batch_generation() {
        let mut params = gen_params();
        params.rollup.target_l2_batch_size = 10;

        let database = get_common_db();

        let blockids = insert_blocks_chainstate(
            55,
            params.rollup.evm_genesis_block_state_root,
            database.as_ref(),
        );

        // no batches
        let batches = generate_batches(blockids[50], database.as_ref(), &params).unwrap();
        assert_eq!(batches, vec![]);

        // single batch
        let batches = generate_batches(blockids[40], database.as_ref(), &params).unwrap();
        assert_eq!(batches, vec![(50, blockids[50])]);

        // multiple batch
        let batches = generate_batches(blockids[20], database.as_ref(), &params).unwrap();
        assert_eq!(
            batches,
            vec![(30, blockids[30]), (40, blockids[40]), (50, blockids[50])]
        );

        // from genesis
        let batches = generate_batches(blockids[0], database.as_ref(), &params).unwrap();
        assert_eq!(
            batches,
            vec![
                (10, blockids[10]),
                (20, blockids[20]),
                (30, blockids[30]),
                (40, blockids[40]),
                (50, blockids[50])
            ]
        );
    }
}
