use std::{collections::HashMap, sync::Arc};

use alpen_express_db::types::{L1TxEntry, L1TxStatus};

use super::{
    error::{BroadcasterError, BroadcasterResult},
    manager::BroadcastManager,
};

pub(crate) struct BroadcasterState {
    /// Next tx idx from which we should next read the tx entries to check and process
    pub(crate) next_idx: u64,

    /// Unfinalized tx entries which the broadcaster will check for
    pub(crate) unfinalized_entries: HashMap<u64, L1TxEntry>,
}

impl BroadcasterState {
    pub async fn initialize(manager: Arc<BroadcastManager>) -> BroadcasterResult<Self> {
        Self::initialize_from_idx(manager, 0).await
    }

    pub async fn initialize_from_idx(
        manager: Arc<BroadcastManager>,
        start_idx: u64,
    ) -> BroadcasterResult<Self> {
        let next_idx = manager
            .get_last_txidx_async()
            .await?
            .map(|x| x + 1)
            .unwrap_or(0);

        let unfinalized_entries = filter_unfinalized_from_db(manager, start_idx, next_idx).await?;

        Ok(Self {
            next_idx,
            unfinalized_entries,
        })
    }

    /// Fetches entries from database based on the `next_idx` and returns a new state
    pub async fn next_state(
        &self,
        updated_entries: HashMap<u64, L1TxEntry>,
        manager: Arc<BroadcastManager>,
    ) -> BroadcasterResult<Self> {
        let mut new_state = Self::initialize_from_idx(manager, self.next_idx).await?;
        if new_state.next_idx < self.next_idx {
            return Err(BroadcasterError::Other(
                "Inconsistent db idx and state idx".to_string(),
            ));
        }
        // Update state
        new_state.unfinalized_entries.extend(updated_entries);
        Ok(new_state)
    }
}

/// Returns unfinalized and unexcluded `[L1TxEntry]`s from db starting from index `from` upto `to`
/// non-inclusive.
async fn filter_unfinalized_from_db(
    manager: Arc<BroadcastManager>,
    from: u64,
    to: u64,
) -> BroadcasterResult<HashMap<u64, L1TxEntry>> {
    let mut unfinalized_entries = HashMap::new();
    for idx in from..to {
        let Some(txentry) = manager.get_txentry_by_idx_async(idx).await? else {
            break;
        };

        match txentry.status {
            L1TxStatus::Finalized | L1TxStatus::Excluded(_) => {}
            _ => {
                unfinalized_entries.insert(idx, txentry);
            }
        }
    }
    Ok(unfinalized_entries)
}
