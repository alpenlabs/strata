use strata_eectl::{engine::ExecEngineCtl, messages::ExecPayloadData};
use strata_storage::NodeStorage;
use tracing::debug;

/// Sync missing blocks in EL using payloads stored in L2 block database.
///
/// TODO: retry on network errors
pub fn sync_chainstate_to_el(
    storage: &NodeStorage,
    engine: &impl ExecEngineCtl,
) -> anyhow::Result<()> {
    let chainstate_manager = storage.chainstate();
    let l2_block_manager = storage.l2();
    let earliest_idx = chainstate_manager.get_earliest_write_idx_blocking()?;
    let latest_idx = chainstate_manager.get_last_write_idx_blocking()?;

    debug!(?earliest_idx, ?latest_idx, "search for last known idx");

    // last idx of chainstate whose corresponding block is present in el
    let sync_from_idx = find_last_match((earliest_idx, latest_idx), |idx| {
        let Some(chain_state) = chainstate_manager.get_toplevel_chainstate_blocking(idx)? else {
            anyhow::bail!(format!("Missing chain state idx: {}", idx));
        };

        let block_id = chain_state.chain_tip_blkid();

        Ok(engine.check_block_exists(*block_id)?)
    })?
    .map(|idx| idx + 1) // sync from next index
    .unwrap_or(0); // sync from genesis

    debug!(?sync_from_idx, "last known index in EL");

    for idx in sync_from_idx..=latest_idx {
        debug!(?idx, "Syncing chainstate");
        let Some(chain_state) = chainstate_manager.get_toplevel_chainstate_blocking(idx)? else {
            anyhow::bail!(format!("Missing chain state idx: {}", idx));
        };

        let block_id = chain_state.chain_tip_blkid();

        let Some(l2block) = l2_block_manager.get_block_data_blocking(block_id)? else {
            anyhow::bail!(format!("Missing L2 block idx: {}", block_id));
        };

        let payload = ExecPayloadData::from_l2_block_bundle(&l2block);

        engine.submit_payload(payload)?;
        engine.update_head_block(*block_id)?;
    }

    Ok(())
}

fn find_last_match<F>(range: (u64, u64), predicate: F) -> anyhow::Result<Option<u64>>
where
    F: Fn(u64) -> anyhow::Result<bool>,
{
    let (mut left, mut right) = range;

    // Check the leftmost value first
    if !predicate(left)? {
        return Ok(None); // If the leftmost value is false, no values can be true
    }

    let mut best_match = None;

    // Proceed with binary search
    while left <= right {
        let mid = left + (right - left) / 2;

        if predicate(mid)? {
            best_match = Some(mid); // Update best match
            left = mid + 1; // Continue searching in the right half
        } else {
            right = mid - 1; // Search in the left half
        }
    }

    Ok(best_match)
}

#[cfg(test)]
mod tests {
    use crate::el_sync::find_last_match;

    #[test]
    fn test_find_last_match() {
        // find match
        assert!(matches!(
            find_last_match((0, 5), |idx| Ok(idx < 3)),
            Ok(Some(2))
        ));
        // found no match
        assert!(matches!(find_last_match((0, 5), |_| Ok(false)), Ok(None)));
        // got error
        let error_message = "intentional error for test";
        assert!(matches!(
            find_last_match((0, 5), |_| anyhow::bail!(error_message)),
            Err(err) if err.to_string() == error_message
        ));
    }
}
