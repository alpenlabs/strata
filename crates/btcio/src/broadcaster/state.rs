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

#[cfg(test)]
mod test {
    use alpen_express_db::{traits::TxBroadcastDatabase, types::ExcludeReason};
    use alpen_express_rocksdb::broadcaster::db::{BroadcastDatabase, BroadcastDb};
    use alpen_test_utils::ArbitraryGenerator;

    use crate::broadcaster::manager::BroadcastManager;

    use super::*;

    fn get_db() -> Arc<impl TxBroadcastDatabase> {
        let db = alpen_test_utils::get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(BroadcastDb::new(db));
        Arc::new(BroadcastDatabase::new(bcastdb))
    }

    fn get_manager() -> Arc<BroadcastManager> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let mgr = BroadcastManager::new(db, Arc::new(pool));
        Arc::new(mgr)
    }

    fn gen_entry_with_status(st: L1TxStatus) -> L1TxEntry {
        let arb = ArbitraryGenerator::new();
        let mut entry: L1TxEntry = arb.generate();
        entry.status = st;
        entry
    }

    fn gen_confirmed_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Confirmed)
    }

    fn gen_finalized_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Finalized)
    }

    fn gen_unpublished_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Unpublished)
    }

    fn gen_published_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Published)
    }

    fn gen_excluded_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Excluded(ExcludeReason::MissingInputsOrSpent))
    }

    async fn populate_broadcast_db(mgr: Arc<BroadcastManager>) -> Vec<(u64, L1TxEntry)> {
        // Make some insertions
        let e1 = gen_unpublished_entry();
        let i1 = mgr
            .add_txentry_async((*e1.txid()).into(), e1.clone())
            .await
            .unwrap();

        let e2 = gen_confirmed_entry();
        let i2 = mgr
            .add_txentry_async((*e2.txid()).into(), e2.clone())
            .await
            .unwrap();

        let e3 = gen_finalized_entry();
        let i3 = mgr
            .add_txentry_async((*e3.txid()).into(), e3.clone())
            .await
            .unwrap();

        let e4 = gen_published_entry();
        let i4 = mgr
            .add_txentry_async((*e4.txid()).into(), e4.clone())
            .await
            .unwrap();

        let e5 = gen_excluded_entry();
        let i5 = mgr
            .add_txentry_async((*e5.txid()).into(), e5.clone())
            .await
            .unwrap();
        vec![(i1, e1), (i2, e2), (i3, e3), (i4, e4), (i5, e5)]
    }

    #[tokio::test]
    async fn test_initialize() {
        // Insert entries to db
        let mgr = get_manager();

        let pop = populate_broadcast_db(mgr.clone()).await;
        let [(i1, _e1), (i2, _e2), (i3, _e3), (i4, _e4), (i5, _e5)] = pop.as_slice() else {
            panic!("Invalid initialization");
        };
        // Now initialize state
        let state = BroadcasterState::initialize(mgr).await.unwrap();

        assert_eq!(state.next_idx, i5 + 1);

        // state should contain all but excluded or finalized entries
        assert!(state.unfinalized_entries.contains_key(i1));
        assert!(state.unfinalized_entries.contains_key(i2));
        assert!(state.unfinalized_entries.contains_key(i4));

        assert!(!state.unfinalized_entries.contains_key(i3));
        assert!(!state.unfinalized_entries.contains_key(i5));
    }

    #[tokio::test]
    async fn test_next_state() {
        // Insert entries to db
        let mgr = get_manager();

        let pop = populate_broadcast_db(mgr.clone()).await;
        let [(_i1, _e1), (_i2, _e2), (_i3, _e3), (_i4, _e4), (_i5, _e5)] = pop.as_slice() else {
            panic!("Invalid initialization");
        };
        // Now initialize state
        let state = BroadcasterState::initialize(mgr.clone()).await.unwrap();

        // Get updated entries where one entry is modified, another is removed
        let mut updated_entries = state.unfinalized_entries.clone();
        let entry = gen_excluded_entry();
        updated_entries.insert(0, entry);
        updated_entries.remove(&1);

        // Insert two more items to db, one excluded and one published.
        let e = gen_excluded_entry(); // this should not be in new state
        let idx = mgr
            .add_txentry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();
        let e1 = gen_published_entry(); // this should be in new state
        let idx1 = mgr
            .add_txentry_async((*e1.txid()).into(), e1.clone())
            .await
            .unwrap();
        // Compute next state
        //
        let newstate = state.next_state(updated_entries, mgr).await.unwrap();

        assert_eq!(newstate.next_idx, idx1 + 1);
        assert_eq!(
            newstate.unfinalized_entries.get(&0).unwrap().status,
            L1TxStatus::Excluded(ExcludeReason::MissingInputsOrSpent)
        );

        // check it does not contain idx of excluded but contains that of published tx
        assert!(!newstate.unfinalized_entries.contains_key(&idx));
        assert!(newstate.unfinalized_entries.contains_key(&idx1));
    }
}
