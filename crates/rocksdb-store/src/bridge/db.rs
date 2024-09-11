use std::sync::Arc;

use alpen_express_db::{
    entities::bridge_tx_state::BridgeTxState,
    errors::DbError,
    traits::{BridgeTxDatabase, BridgeTxProvider, BridgeTxStore},
    DbResult,
};
use alpen_express_primitives::buf::Buf32;
use rockbound::{OptimisticTransactionDB as DB, SchemaDBOperationsExt, TransactionRetry};

use super::schemas::{BridgeTxStateSchema, BridgeTxStateTxidSchema};
use crate::DbOpsConfig;

pub struct BridgeTxRdbStore {
    db: Arc<DB>,
    ops: DbOpsConfig,
}

impl BridgeTxRdbStore {
    pub fn new(db: Arc<DB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl BridgeTxStore for BridgeTxRdbStore {
    fn upsert_tx_state(&self, txid: Buf32, tx_state: BridgeTxState) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                // insert new id if the txid is new
                if txn.get::<BridgeTxStateSchema>(&txid)?.is_none() {
                    let idx = rockbound::utils::get_last::<BridgeTxStateTxidSchema>(txn)?
                        .map(|(x, _)| x + 1)
                        .unwrap_or(0);

                    txn.put::<BridgeTxStateTxidSchema>(&idx, &txid)?;
                }

                txn.put::<BridgeTxStateSchema>(&txid, &tx_state)?;

                Ok::<(), DbError>(())
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }

    fn evict_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                if let Some(state) = txn.get::<BridgeTxStateSchema>(&txid)? {
                    txn.delete::<BridgeTxStateSchema>(&txid)?;
                    return Ok::<Option<BridgeTxState>, DbError>(Some(state));
                }

                Ok(None)
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }
}

pub struct BridgeTxRdbProvider {
    db: Arc<DB>,
}

impl BridgeTxRdbProvider {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

impl BridgeTxProvider for BridgeTxRdbProvider {
    fn get_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        Ok(self.db.get::<BridgeTxStateSchema>(&txid)?)
    }
}

pub struct BridgeTxRocksDb {
    store: Arc<BridgeTxRdbStore>,
    provider: Arc<BridgeTxRdbProvider>,
}

impl BridgeTxRocksDb {
    pub fn new(store: Arc<BridgeTxRdbStore>, provider: Arc<BridgeTxRdbProvider>) -> Self {
        Self { store, provider }
    }
}

impl BridgeTxDatabase for BridgeTxRocksDb {
    type Store = BridgeTxRdbStore;
    type Provider = BridgeTxRdbProvider;

    fn bridge_tx_store(&self) -> &Arc<Self::Store> {
        &self.store
    }

    fn bridge_tx_provider(&self) -> &Arc<Self::Provider> {
        &self.provider
    }
}

#[cfg(test)]
mod tests {
    use alpen_express_db::traits::BridgeTxDatabase;
    use arbitrary::{Arbitrary, Unstructured};

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_bridge_tx_state_store() {
        let db = setup_db();

        let store = db.bridge_tx_store();
        let provider = db.bridge_tx_provider();

        let raw_bytes = vec![0u8; 1024];
        let mut u = Unstructured::new(&raw_bytes);

        let bridge_tx_state = BridgeTxState::arbitrary(&mut u).unwrap();

        let txid = bridge_tx_state.compute_txid().into();

        // Test insert
        let result = store.upsert_tx_state(txid, bridge_tx_state.clone());
        assert!(
            result.is_ok(),
            "should be able to add collected sigs but got: {}",
            result.err().unwrap()
        );

        // Test read
        let stored_entry = provider.get_tx_state(txid);
        assert!(
            stored_entry.is_ok(),
            "should be able to access stored entry but got: {}",
            stored_entry.err().unwrap()
        );

        let stored_entry = stored_entry.unwrap();
        assert_eq!(
            stored_entry,
            Some(bridge_tx_state),
            "stored entity should match the entity being stored"
        );

        // Test update
        let new_state = BridgeTxState::arbitrary(&mut u).unwrap();
        let result = store.upsert_tx_state(txid, new_state.clone());
        assert!(
            result.is_ok(),
            "should be able to update existing data at a given Txid but got: {}",
            result.err().unwrap()
        );

        let stored_entry = provider.get_tx_state(txid);
        assert!(
            stored_entry.is_ok(),
            "should be able to access updated stored entry but got: {}",
            stored_entry.err().unwrap()
        );

        let stored_entry = stored_entry.unwrap();
        assert_eq!(
            stored_entry,
            Some(new_state),
            "stored entity should match the updated entity being stored"
        );

        // Test evict
        let evicted_entry = store.evict_tx_state(txid).unwrap();
        assert!(
            evicted_entry.is_some_and(|entry| entry == stored_entry.unwrap()),
            "stored entry should be returned after being evicted"
        );

        let re_evicted_entry = store.evict_tx_state(txid).unwrap();
        assert!(
            re_evicted_entry.is_none(),
            "evicting an already evicted entry should return None"
        );

        let stored_entry = provider.get_tx_state(txid).unwrap();
        assert!(
            stored_entry.is_none(),
            "stored entry should not be present after eviction"
        );
    }

    fn setup_db() -> BridgeTxRocksDb {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        let store = BridgeTxRdbStore::new(db.clone(), db_ops);
        let provider = BridgeTxRdbProvider::new(db.clone());

        BridgeTxRocksDb::new(Arc::new(store), Arc::new(provider))
    }
}
