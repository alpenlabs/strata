use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{ClientStateProvider, Database, L2DataProvider},
};
use alpen_express_primitives::params::RollupParams;
use alpen_express_state::{block::L2BlockBundle, client_state::ClientState, id::L2BlockId};
use anyhow::{Context, Result};
use reth_primitives::B256;
use reth_rpc_types::engine::ForkchoiceState;

use crate::block::EVML2Block;

pub fn fork_choice_state_initial<D: Database>(
    db: Arc<D>,
    config: &RollupParams,
) -> Result<ForkchoiceState> {
    let last_cstate = get_last_checkpoint_state(db.as_ref())?;

    let latest_block_hash = get_block_hash_by_id(
        db.as_ref(),
        last_cstate
            .as_ref()
            .and_then(|state| state.sync())
            .map(|sync_state| sync_state.chain_tip_blkid()),
    )?
    .unwrap_or(config.evm_genesis_block_hash.into());

    let finalized_block_hash = get_block_hash_by_id(
        db.as_ref(),
        last_cstate
            .as_ref()
            .and_then(|state| state.sync())
            .map(|sync_state| sync_state.finalized_blkid()),
    )?
    .unwrap_or(B256::ZERO);

    Ok(ForkchoiceState {
        head_block_hash: latest_block_hash,
        safe_block_hash: latest_block_hash,
        finalized_block_hash,
    })
}

fn get_block_hash(l2_block: L2BlockBundle) -> Result<B256> {
    EVML2Block::try_from(l2_block)
        .map(|block| block.block_hash())
        .context("Failed to convert L2Block to EVML2Block")
}

fn get_last_checkpoint_state<D: Database>(db: &D) -> Result<Option<ClientState>> {
    let last_checkpoint_idx = db.client_state_provider().get_last_checkpoint_idx();

    if let Err(DbError::NotBootstrapped) = last_checkpoint_idx {
        // before genesis block ready; use hardcoded genesis state
        return Ok(None);
    }

    last_checkpoint_idx
        .and_then(|ckpt_idx| db.client_state_provider().get_state_checkpoint(ckpt_idx))
        .context("Failed to get last checkpoint state")
}

fn get_block_hash_by_id<D: Database>(
    db: &D,
    block_id: Option<&L2BlockId>,
) -> anyhow::Result<Option<B256>> {
    block_id
        .and_then(|id| db.l2_provider().get_block_data(*id).transpose())
        .transpose()
        .context("Failed to get block data")?
        .map(get_block_hash)
        .transpose()
}
