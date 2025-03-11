// TODO this all needs to be reworked to just follow what the FCM state
// publishing is, waiting for that to be ready before getting started

use anyhow::{Context, Result};
use revm_primitives::B256;
use strata_db::{errors::DbError, traits::BlockStatus};
use strata_primitives::{
    l2::{L2BlockCommitment, L2BlockId},
    params::RollupParams,
};
use strata_state::{block::L2BlockBundle, chain_state::Chainstate};
use strata_storage::*;
use tracing::*;

use crate::block::EVML2Block;

/// Horrible bodgy function to try to figure out the highest valid block we have.
///
/// This makes some tenuous assumptions that will probably be violated in the
/// future.  It assumes that we will never have conflicting L2 blocks in a slot.
/// It also depends on chainstate manager interfaces that will probably be
/// removed in the near future.
///
/// This should no longer be necessary after STR-17.
pub fn fetch_init_fork_choice_state(
    storage: &NodeStorage,
    rollup_params: &RollupParams,
) -> Result<B256> {
    let l2man = storage.l2();

    match get_last_chainstate(storage)? {
        Some(last_chs) => {
            let slot = last_chs.chain_tip_slot();

            // Find the block we're looking for.
            let tip_block = find_most_recent_valid_l2_block_as_of(slot, l2man)?;
            info!(
                ?tip_block,
                "found likely safe tip block for EVM to resume from"
            );

            compute_evm_fc_state_from_chainstate(&tip_block, storage)
        }
        None => {
            info!("preparing EVM initial state from genesis");
            let evm_genesis_block_hash =
                revm_primitives::FixedBytes(*rollup_params.evm_genesis_block_hash.as_ref());
            Ok(evm_genesis_block_hash)
        }
    }
}

fn find_most_recent_valid_l2_block_as_of(
    slot: u64,
    l2man: &L2BlockManager,
) -> Result<L2BlockCommitment> {
    let limit = 64;

    for i in 0..limit {
        let Some(ck_slot) = slot.checked_sub(i) else {
            // Horrible, whatever.
            anyhow::bail!("underflowed slot while searching for valid block");
        };

        // We *should* always find it on the first block we look at, but we
        // can't guarantee that.
        let blkids = l2man.get_blocks_at_height_blocking(ck_slot)?;
        for b in blkids {
            let Some(status) = l2man.get_block_status_blocking(&b)? else {
                continue;
            };

            if status == BlockStatus::Valid {
                return Ok(L2BlockCommitment::new(ck_slot, b));
            }
        }
    }

    anyhow::bail!("could not find chain tip block");
}

fn compute_evm_fc_state_from_chainstate(
    block: &L2BlockCommitment,
    storage: &NodeStorage,
) -> Result<B256> {
    let l2man = storage.l2();
    let latest_evm_block_hash =
        get_evm_block_hash_by_id(block.blkid(), l2man)?.expect("evmexec: missing expected block");
    Ok(latest_evm_block_hash)
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
