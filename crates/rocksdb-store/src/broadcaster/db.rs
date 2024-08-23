use std::sync::Arc;

use alpen_express_db::{
    errors::DbError,
    traits::{BcastProvider, BcastStore, TxBroadcastDatabase},
    types::L1TxEntry,
    DbResult,
};
use alpen_express_primitives::buf::Buf32;
use rockbound::{
    utils::get_last, OptimisticTransactionDB as DB, SchemaDBOperationsExt, TransactionRetry,
};

use super::schemas::{BcastL1TxIdSchema, BcastL1TxSchema};
use crate::DbOpsConfig;

pub struct BroadcastDb {
    db: Arc<DB>,
    ops: DbOpsConfig,
}

impl BroadcastDb {
    pub fn new(db: Arc<DB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl BcastStore for BroadcastDb {
    fn insert_new_tx_entry(&self, txid: Buf32, txentry: L1TxEntry) -> DbResult<u64> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                if txn.get::<BcastL1TxSchema>(&txid)?.is_some() {
                    return Err(DbError::Other(format!(
                        "Entry already exists for id {txid:?}"
                    )));
                }

                let idx = rockbound::utils::get_last::<BcastL1TxIdSchema>(txn)?
                    .map(|(x, _)| x + 1)
                    .unwrap_or(0);

                txn.put::<BcastL1TxIdSchema>(&idx, &txid)?;
                txn.put::<BcastL1TxSchema>(&txid, &txentry)?;

                Ok(idx)
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn update_tx_entry_by_id(&self, txid: Buf32, txentry: L1TxEntry) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if tx.get::<BcastL1TxSchema>(&txid)?.is_none() {
                    return Err(DbError::Other(format!(
                        "Entry does not exist for id {txid:?}"
                    )));
                }
                Ok(tx.put::<BcastL1TxSchema>(&txid, &txentry)?)
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn update_tx_entry(
        &self,
        idx: u64,
        txentry: alpen_express_db::types::L1TxEntry,
    ) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |tx| {
                if let Some(id) = tx.get::<BcastL1TxIdSchema>(&idx)? {
                    Ok(tx.put::<BcastL1TxSchema>(&id, &txentry)?)
                } else {
                    Err(DbError::Other(format!(
                        "Entry does not exist for idx {idx:?}"
                    )))
                }
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }
}

impl BcastProvider for BroadcastDb {
    fn get_tx_entry_by_id(&self, txid: Buf32) -> DbResult<Option<L1TxEntry>> {
        Ok(self.db.get::<BcastL1TxSchema>(&txid)?)
    }

    fn get_next_tx_idx(&self) -> DbResult<u64> {
        Ok(get_last::<BcastL1TxIdSchema>(self.db.as_ref())?
            .map(|(k, _)| k + 1)
            .unwrap_or_default())
    }

    fn get_txid(&self, idx: u64) -> DbResult<Option<Buf32>> {
        Ok(self.db.get::<BcastL1TxIdSchema>(&idx)?)
    }

    fn get_tx_entry(&self, idx: u64) -> DbResult<Option<L1TxEntry>> {
        if let Some(id) = self.get_txid(idx)? {
            Ok(self.db.get::<BcastL1TxSchema>(&id)?)
        } else {
            Err(DbError::Other(format!(
                "Entry does not exist for idx {idx:?}"
            )))
        }
    }
}

pub struct BroadcastDatabase {
    db: Arc<BroadcastDb>,
}

impl BroadcastDatabase {
    pub fn new(db: Arc<BroadcastDb>) -> Self {
        Self { db }
    }
}

impl TxBroadcastDatabase for BroadcastDatabase {
    type BcastStore = BroadcastDb;
    type BcastProv = BroadcastDb;

    fn broadcast_store(&self) -> &Arc<Self::BcastStore> {
        &self.db
    }

    fn broadcast_provider(&self) -> &Arc<Self::BcastProv> {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use alpen_express_db::{
        errors::DbError,
        traits::{BcastProvider, BcastStore},
        types::L1TxStatus,
    };
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::bitcoin::get_test_bitcoin_txns;
    use bitcoin::hashes::Hash;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    fn setup_db() -> BroadcastDb {
        let (db, db_ops) = get_rocksdb_tmp_instance().unwrap();
        BroadcastDb::new(db, db_ops)
    }

    fn generate_l1_tx_entry() -> (Buf32, L1TxEntry) {
        let txns = get_test_bitcoin_txns();
        let txid = txns[0].compute_txid().as_raw_hash().to_byte_array().into();
        let txentry = L1TxEntry::from_tx(&txns[0]);
        (txid, txentry)
    }

    #[test]
    fn test_add_tx_new_entry() {
        let db = setup_db();

        let (txid, txentry) = generate_l1_tx_entry();

        let idx = db.insert_new_tx_entry(txid, txentry.clone()).unwrap();

        assert_eq!(idx, 0);

        let stored_entry = db.get_tx_entry(idx).unwrap();
        assert_eq!(stored_entry, Some(txentry));
    }

    #[test]
    fn test_add_tx_existing_entry() {
        let broadcast_db = setup_db();

        let (txid, txentry) = generate_l1_tx_entry();

        let _ = broadcast_db
            .insert_new_tx_entry(txid, txentry.clone())
            .unwrap();

        let result = broadcast_db.insert_new_tx_entry(txid, txentry);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Entry already exists for id"));
        }
    }

    #[test]
    fn test_update_tx() {
        let broadcast_db = setup_db();

        let (txid, txentry) = generate_l1_tx_entry();

        // Attempt to update non-existing entry
        let result = broadcast_db.update_tx_entry_by_id(txid, txentry.clone());
        assert!(result.is_err());

        // Add and then update the entry
        let _ = broadcast_db
            .insert_new_tx_entry(txid, txentry.clone())
            .unwrap();

        let mut updated_txentry = txentry;
        updated_txentry.status = L1TxStatus::Finalized { confirmations: 1 };

        broadcast_db
            .update_tx_entry_by_id(txid, updated_txentry.clone())
            .unwrap();

        let stored_entry = broadcast_db.get_tx_entry_by_id(txid).unwrap();
        assert_eq!(stored_entry, Some(updated_txentry));
    }

    #[test]
    fn test_update_tx_entry() {
        let broadcast_db = setup_db();

        let (txid, txentry) = generate_l1_tx_entry();

        // Attempt to update non-existing index
        let result = broadcast_db.update_tx_entry(0, txentry.clone());
        assert!(result.is_err());

        // Add and then update the entry by index
        let idx = broadcast_db
            .insert_new_tx_entry(txid, txentry.clone())
            .unwrap();

        let mut updated_txentry = txentry;
        updated_txentry.status = L1TxStatus::Finalized { confirmations: 1 };

        broadcast_db
            .update_tx_entry(idx, updated_txentry.clone())
            .unwrap();

        let stored_entry = broadcast_db.get_tx_entry(idx).unwrap();
        assert_eq!(stored_entry, Some(updated_txentry));
    }

    #[test]
    fn test_get_txentry_by_idx() {
        let broadcast_db = setup_db();

        // Test non-existing entry
        let result = broadcast_db.get_tx_entry(0);
        assert!(result.is_err());

        let (txid, txentry) = generate_l1_tx_entry();

        let idx = broadcast_db
            .insert_new_tx_entry(txid, txentry.clone())
            .unwrap();

        let stored_entry = broadcast_db.get_tx_entry(idx).unwrap();
        assert_eq!(stored_entry, Some(txentry));
    }

    #[test]
    fn test_get_next_txidx() {
        let broadcast_db = setup_db();

        let next_txidx = broadcast_db.get_next_tx_idx().unwrap();
        assert_eq!(next_txidx, 0, "The next txidx is 0 in the beginning");

        let (txid, txentry) = generate_l1_tx_entry();

        let idx = broadcast_db
            .insert_new_tx_entry(txid, txentry.clone())
            .unwrap();

        let next_txidx = broadcast_db.get_next_tx_idx().unwrap();

        assert_eq!(next_txidx, idx + 1);
    }
}
