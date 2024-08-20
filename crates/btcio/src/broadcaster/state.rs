use std::{collections::HashMap, sync::Arc};

use alpen_express_db::types::{L1TxEntry, L1TxStatus};
use express_storage::managers::l1tx_broadcast::BroadcastDbManager;

use super::error::{BroadcasterError, BroadcasterResult};

pub(crate) struct BroadcasterState {
    /// Next index from which we should next read the [`L1TxEntry`] to check and process
    pub(crate) next_idx: u64,

    /// Unfinalized [`L1TxEntry`]s which the broadcaster will check for
    pub(crate) unfinalized_entries: HashMap<u64, L1TxEntry>,
}

impl BroadcasterState {
    /// Initialize the `[BroadcasterState]` by looking at all [`L1TxEntry`]s in database
    pub async fn initialize(manager: &Arc<BroadcastDbManager>) -> BroadcasterResult<Self> {
        Self::initialize_from_idx(manager, 0).await
    }

    /// Initialize the [`BroadcasterState`] by looking at [`L1TxEntry`]s in database starting from
    /// given `start_idx`
    pub async fn initialize_from_idx(
        manager: &Arc<BroadcastDbManager>,
        start_idx: u64,
    ) -> BroadcasterResult<Self> {
        let next_idx = manager.get_next_tx_idx_async().await?;

        let unfinalized_entries = filter_unfinalized_from_db(manager, start_idx, next_idx).await?;

        Ok(Self {
            next_idx,
            unfinalized_entries,
        })
    }

    /// Fetches entries from database based on the `next_idx` and updates the broadcaster state
    pub async fn next(
        &mut self,
        updated_entries: HashMap<u64, L1TxEntry>,
        manager: &Arc<BroadcastDbManager>,
    ) -> BroadcasterResult<()> {
        let next_idx = manager.get_next_tx_idx_async().await?;

        if next_idx < self.next_idx {
            return Err(BroadcasterError::Other(
                "Inconsistent db idx and state idx".to_string(),
            ));
        }
        let new_unfinalized_entries =
            filter_unfinalized_from_db(manager, self.next_idx, next_idx).await?;

        // Update state: include updated entries and new unfinalized entries
        self.unfinalized_entries.extend(updated_entries);
        self.unfinalized_entries.extend(new_unfinalized_entries);
        self.next_idx = next_idx;
        Ok(())
    }
}

/// Returns unfinalized and unexcluded [`L1TxEntry`]s from db starting from index `from` until `to`
/// non-inclusive.
async fn filter_unfinalized_from_db(
    manager: &Arc<BroadcastDbManager>,
    from: u64,
    to: u64,
) -> BroadcasterResult<HashMap<u64, L1TxEntry>> {
    let mut unfinalized_entries = HashMap::new();
    for idx in from..to {
        let Some(txentry) = manager.get_tx_entry_async(idx).await? else {
            break;
        };

        match txentry.status {
            L1TxStatus::Finalized(_) | L1TxStatus::Excluded(_) => {}
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
    use alpen_express_rocksdb::{
        broadcaster::db::{BroadcastDatabase, BroadcastDb},
        test_utils::get_rocksdb_tmp_instance,
    };
    use alpen_test_utils::ArbitraryGenerator;
    use express_storage::managers::l1tx_broadcast::L1BroadcastContext;

    use super::*;

    fn get_db() -> Arc<impl TxBroadcastDatabase> {
        let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(BroadcastDb::new(db, dbops));
        Arc::new(BroadcastDatabase::new(bcastdb))
    }

    fn get_manager() -> Arc<BroadcastDbManager> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let mgr = L1BroadcastContext::new(db).into_ops(pool);
        Arc::new(mgr)
    }

    fn gen_entry_with_status(st: L1TxStatus) -> L1TxEntry {
        let arb = ArbitraryGenerator::new();
        let mut entry: L1TxEntry = arb.generate();
        entry.status = st;
        entry
    }

    fn gen_confirmed_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Confirmed(1))
    }

    fn gen_finalized_entry() -> L1TxEntry {
        gen_entry_with_status(L1TxStatus::Finalized(1))
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

    async fn populate_broadcast_db(mgr: Arc<BroadcastDbManager>) -> Vec<(u64, L1TxEntry)> {
        // Make some insertions
        let e1 = gen_unpublished_entry();
        let i1 = mgr
            .insert_new_tx_entry_async((*e1.txid()).into(), e1.clone())
            .await
            .unwrap();

        let e2 = gen_confirmed_entry();
        let i2 = mgr
            .insert_new_tx_entry_async((*e2.txid()).into(), e2.clone())
            .await
            .unwrap();

        let e3 = gen_finalized_entry();
        let i3 = mgr
            .insert_new_tx_entry_async((*e3.txid()).into(), e3.clone())
            .await
            .unwrap();

        let e4 = gen_published_entry();
        let i4 = mgr
            .insert_new_tx_entry_async((*e4.txid()).into(), e4.clone())
            .await
            .unwrap();

        let e5 = gen_excluded_entry();
        let i5 = mgr
            .insert_new_tx_entry_async((*e5.txid()).into(), e5.clone())
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
        let state = BroadcasterState::initialize(&mgr).await.unwrap();

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
        let mut state = BroadcasterState::initialize(&mgr).await.unwrap();

        // Get updated entries where one entry is modified, another is removed
        let mut updated_entries = state.unfinalized_entries.clone();
        let entry = gen_excluded_entry();
        updated_entries.insert(0, entry);
        updated_entries.remove(&1);

        // Insert two more items to db, one excluded and one published.
        let e = gen_excluded_entry(); // this should not be in new state
        let idx = mgr
            .insert_new_tx_entry_async((*e.txid()).into(), e.clone())
            .await
            .unwrap();
        let e1 = gen_published_entry(); // this should be in new state
        let idx1 = mgr
            .insert_new_tx_entry_async((*e1.txid()).into(), e1.clone())
            .await
            .unwrap();
        // Compute next state
        //
        state.next(updated_entries, &mgr).await.unwrap();

        assert_eq!(state.next_idx, idx1 + 1);
        assert_eq!(
            state.unfinalized_entries.get(&0).unwrap().status,
            L1TxStatus::Excluded(ExcludeReason::MissingInputsOrSpent)
        );

        // check it does not contain idx of excluded but contains that of published tx
        assert!(!state.unfinalized_entries.contains_key(&idx));
        assert!(state.unfinalized_entries.contains_key(&idx1));
    }
}
