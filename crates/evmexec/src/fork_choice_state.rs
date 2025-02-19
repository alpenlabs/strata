// TODO this all needs to be reworked to just follow what the FCM state
// publishing is, waiting for that to be ready before getting started

use alloy_rpc_types::engine::ForkchoiceState;
use anyhow::{Context, Result};
use revm_primitives::B256;
use strata_db::errors::DbError;
use strata_primitives::params::RollupParams;
use strata_state::{block::L2BlockBundle, chain_state::Chainstate, id::L2BlockId};
use strata_storage::*;
use tracing::*;

use crate::block::EVML2Block;

pub fn fetch_init_fork_choice_state(
    storage: &NodeStorage,
    rollup_params: &RollupParams,
) -> Result<ForkchoiceState> {
    // TODO switch these logs to debug
    match get_last_chainstate(storage)? {
        Some(chs) => {
            let tip = chs.chain_tip_blkid();
            let slot = chs.chain_tip_slot();
            info!(%slot, %tip, "preparing EVM initial state from chainstate");
            compute_evm_fc_state_from_chainstate(&chs, storage)
        }
        None => {
            info!("preparing EVM initial state from genesis");
            let genesis =
                revm_primitives::FixedBytes(*rollup_params.evm_genesis_block_hash.as_ref());
            Ok(ForkchoiceState {
                head_block_hash: genesis,
                safe_block_hash: genesis,
                finalized_block_hash: B256::ZERO,
            })
        }
    }
}

fn compute_evm_fc_state_from_chainstate(
    chs: &Chainstate,
    storage: &NodeStorage,
) -> Result<ForkchoiceState> {
    let l2man = storage.l2();
    let latest_block_hash = get_evm_block_hash_by_id(chs.chain_tip_blkid(), l2man)?
        .expect("evmexec: missing expected block");
    let finalized_block_hash = if chs.finalized_epoch().is_null() {
        // no finalized epoch present yet
        B256::ZERO
    } else {
        get_evm_block_hash_by_id(chs.finalized_epoch().last_blkid(), l2man)?
            .expect("evmexec: missing expected block")
    };
    Ok(ForkchoiceState {
        head_block_hash: latest_block_hash,
        safe_block_hash: latest_block_hash,
        finalized_block_hash,
    })
}

fn get_last_chainstate(storage: &NodeStorage) -> Result<Option<Chainstate>> {
    let chsman = storage.chainstate();

    let last_write_idx = match chsman.get_last_write_idx_blocking() {
        Ok(idx) => idx,
        Err(DbError::NotBootstrapped) => {
            // before genesis block ready; use hardcoded genesis state
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };

    Ok(chsman.get_toplevel_chainstate_blocking(last_write_idx)?)
}

fn get_evm_block_hash_by_id(
    block_id: &L2BlockId,
    l2man: &L2BlockManager,
) -> anyhow::Result<Option<B256>> {
    l2man
        .get_block_data_blocking(block_id)?
        .map(|bundle| compute_evm_block_hash(&bundle))
        .transpose()
}

fn compute_evm_block_hash(l2_block: &L2BlockBundle) -> Result<B256> {
    EVML2Block::try_extract(l2_block)
        .map(|block| block.block_hash())
        .context("Failed to convert L2Block to EVML2Block")
}
