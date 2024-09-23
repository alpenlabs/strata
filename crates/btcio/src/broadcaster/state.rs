use std::{collections::BTreeMap, sync::Arc};

use alpen_express_db::types::L1TxEntry;
use bitcoin::Txid;
use express_storage::BroadcastDbOps;
use tracing::*;

use super::error::{BroadcasterError, BroadcasterResult};

pub(crate) struct BroadcasterState {
    /// Next index from which we should next read the [`L1TxEntry`] to check and process
    pub(crate) next_idx: u64,

    /// Unfinalized [`L1TxEntry`]s which the broadcaster will check for
    pub(crate) unfinalized_entries: BTreeMap<u64, L1TxEntry>,
}

impl BroadcasterState {
    /// Initialize the `[BroadcasterState]` by looking at all [`L1TxEntry`]s in database
    pub async fn initialize(ops: &Arc<BroadcastDbOps>) -> BroadcasterResult<Self> {
        Self::initialize_from_idx(ops, 0).await
    }

    /// Initialize the [`BroadcasterState`] by looking at [`L1TxEntry`]s in database starting from
    /// given `start_idx`
    pub async fn initialize_from_idx(
        ops: &Arc<BroadcastDbOps>,
        start_idx: u64,
    ) -> BroadcasterResult<Self> {
        let next_idx = ops.get_next_tx_idx_async().await?;

        let unfinalized_entries = filter_unfinalized_from_db(ops, start_idx, next_idx).await?;

        Ok(Self {
            next_idx,
            unfinalized_entries,
        })
    }

    /// Fetches entries from database based on the `next_idx` and updates the broadcaster state
    pub async fn next(
        &mut self,
        updated_entries: BTreeMap<u64, L1TxEntry>,
        ops: &Arc<BroadcastDbOps>,
    ) -> BroadcasterResult<()> {
        let next_idx = ops.get_next_tx_idx_async().await?;

        if next_idx < self.next_idx {
            return Err(BroadcasterError::Other(
                "Inconsistent db idx and state idx".to_string(),
            ));
        }
        let new_unfinalized_entries =
            filter_unfinalized_from_db(ops, self.next_idx, next_idx).await?;

        // Update state: include updated entries and new unfinalized entries
        self.unfinalized_entries.extend(updated_entries);
        self.unfinalized_entries.extend(new_unfinalized_entries);
        self.next_idx = next_idx;
        Ok(())
    }
}

/// Returns unfinalized but valid [`L1TxEntry`]s from db starting from index `from` until `to`
/// non-inclusive.
async fn filter_unfinalized_from_db(
    ops: &Arc<BroadcastDbOps>,
    from: u64,
    to: u64,
) -> BroadcasterResult<BTreeMap<u64, L1TxEntry>> {
    let mut unfinalized_entries = BTreeMap::new();
    for idx in from..to {
        let Some(txentry) = ops.get_tx_entry_async(idx).await? else {
            break;
        };

        let status = &txentry.status;
        let txid = ops.get_txid_async(idx).await?.map(Txid::from);
        debug!(?idx, ?txid, ?status, "TxEntry");

        if txentry.is_valid() && !txentry.is_finalized() {
            unfinalized_entries.insert(idx, txentry);
        }
    }
    Ok(unfinalized_entries)
}

#[cfg(test)]
mod test {
    use alpen_express_db::{traits::L1BroadcastDatabase, types::L1TxStatus};
    use alpen_express_rocksdb::{
        broadcaster::db::{BroadcastDatabase, L1BroadcastDb},
        test_utils::get_rocksdb_tmp_instance,
    };
    use bitcoin::{consensus, Transaction};
    use express_storage::ops::l1tx_broadcast::Context;

    use super::*;
    use crate::test_utils::SOME_TX;

    fn get_db() -> Arc<impl L1BroadcastDatabase> {
        let (db, dbops) = get_rocksdb_tmp_instance().unwrap();
        let bcastdb = Arc::new(L1BroadcastDb::new(db, dbops));
        Arc::new(BroadcastDatabase::new(bcastdb))
    }

    fn get_ops() -> Arc<BroadcastDbOps> {
        let pool = threadpool::Builder::new().num_threads(2).build();
        let db = get_db();
        let ops = Context::new(db).into_ops(pool);
        Arc::new(ops)
    }

    fn gen_entry_with_status(st: L1TxStatus) -> L1TxEntry {
        let tx: Transaction = consensus::encode::deserialize_hex(SOME_TX).unwrap();
        let mut entry = L1TxEntry::from_tx(&tx);
        entry.status = st;
        entry
    }

    async fn populate_broadcast_db(ops: Arc<BroadcastDbOps>) -> Vec<(u64, L1TxEntry)> {
        // Make some insertions
        let e1 = gen_entry_with_status(L1TxStatus::Unpublished);
        let i1 = ops
            .put_tx_entry_async([1; 32].into(), e1.clone())
            .await
            .unwrap();

        let e2 = gen_entry_with_status(L1TxStatus::Confirmed { confirmations: 1 });
        let i2 = ops
            .put_tx_entry_async([2; 32].into(), e2.clone())
            .await
            .unwrap();

        let e3 = gen_entry_with_status(L1TxStatus::Finalized { confirmations: 1 });
        let i3 = ops
            .put_tx_entry_async([3; 32].into(), e3.clone())
            .await
            .unwrap();

        let e4 = gen_entry_with_status(L1TxStatus::Published);
        let i4 = ops
            .put_tx_entry_async([4; 32].into(), e4.clone())
            .await
            .unwrap();

        let e5 = gen_entry_with_status(L1TxStatus::InvalidInputs);
        let i5 = ops
            .put_tx_entry_async([5; 32].into(), e5.clone())
            .await
            .unwrap();
        vec![
            (i1.unwrap(), e1),
            (i2.unwrap(), e2),
            (i3.unwrap(), e3),
            (i4.unwrap(), e4),
            (i5.unwrap(), e5),
        ]
    }

    #[tokio::test]
    async fn test_initialize() {
        // Insert entries to db
        let ops = get_ops();

        let pop = populate_broadcast_db(ops.clone()).await;
        let [(i1, _e1), (i2, _e2), (i3, _e3), (i4, _e4), (i5, _e5)] = pop.as_slice() else {
            panic!("Invalid initialization");
        };
        // Now initialize state
        let state = BroadcasterState::initialize(&ops).await.unwrap();

        assert_eq!(state.next_idx, i5 + 1);

        // state should contain all except reorged, invalid or  finalized entries
        assert!(state.unfinalized_entries.contains_key(i1));
        assert!(state.unfinalized_entries.contains_key(i2));
        assert!(state.unfinalized_entries.contains_key(i4));

        assert!(!state.unfinalized_entries.contains_key(i3));
        assert!(!state.unfinalized_entries.contains_key(i5));
    }

    #[tokio::test]
    async fn test_next_state() {
        // Insert entries to db
        let ops = get_ops();

        let pop = populate_broadcast_db(ops.clone()).await;
        let [(_i1, _e1), (_i2, _e2), (_i3, _e3), (_i4, _e4), (_i5, _e5)] = pop.as_slice() else {
            panic!("Invalid initialization");
        };
        // Now initialize state
        let mut state = BroadcasterState::initialize(&ops).await.unwrap();

        // Get updated entries where one entry is modified, another is removed
        let mut updated_entries = state.unfinalized_entries.clone();
        let entry = gen_entry_with_status(L1TxStatus::InvalidInputs);
        updated_entries.insert(0, entry);
        updated_entries.remove(&1);

        // Insert two more items to db, one excluded and one published. Note the new idxs than used
        // in populate db.
        let e = gen_entry_with_status(L1TxStatus::InvalidInputs);
        let idx = ops
            .put_tx_entry_async([7; 32].into(), e.clone())
            .await
            .unwrap();

        let e1 = gen_entry_with_status(L1TxStatus::Published); // this should be in new state
        let idx1 = ops
            .put_tx_entry_async([8; 32].into(), e1.clone())
            .await
            .unwrap();
        // Compute next state
        //
        state.next(updated_entries, &ops).await.unwrap();

        assert_eq!(state.next_idx, idx1.unwrap() + 1);
        assert_eq!(
            state.unfinalized_entries.get(&0).unwrap().status,
            L1TxStatus::InvalidInputs
        );

        // check it does not contain idx of reorged but contains that of published tx
        assert!(!state.unfinalized_entries.contains_key(&idx.unwrap()));
        assert!(state.unfinalized_entries.contains_key(&idx1.unwrap()));
    }
}
