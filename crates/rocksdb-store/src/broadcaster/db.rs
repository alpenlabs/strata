use std::sync::Arc;

use rockbound::{
    utils::get_last, OptimisticTransactionDB as DB, SchemaDBOperationsExt, TransactionRetry,
};

use alpen_express_db::{
    errors::DbError,
    traits::{BcastProvider, BcastStore},
    types::L1TxEntry,
    DbResult,
};
use alpen_express_primitives::buf::Buf32;

use super::schemas::{BcastL1TxIdSchema, BcastL1TxSchema};

pub struct BroadcastDb {
    db: Arc<DB>,
}

impl BroadcastDb {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }
}

const RETRY: TransactionRetry = TransactionRetry::Count(5);

impl BcastStore for BroadcastDb {
    fn add_tx(&self, txid: Buf32, txentry: L1TxEntry) -> DbResult<u64> {
        self.db
            .with_optimistic_txn(RETRY, |txn| {
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

    fn update_tx(&self, txid: Buf32, txentry: L1TxEntry) -> DbResult<()> {
        self.db
            .with_optimistic_txn(RETRY, |tx| {
                if tx.get::<BcastL1TxSchema>(&txid)?.is_none() {
                    return Err(DbError::Other(format!(
                        "Entry does not exist for id {txid:?}"
                    )));
                }
                Ok(tx.put::<BcastL1TxSchema>(&txid, &txentry)?)
            })
            .map_err(|e| DbError::TransactionError(e.to_string()))
    }

    fn update_tx_by_idx(
        &self,
        idx: u64,
        txentry: alpen_express_db::types::L1TxEntry,
    ) -> DbResult<()> {
        self.db
            .with_optimistic_txn(RETRY, |tx| {
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
    fn get_txentry(&self, txid: Buf32) -> DbResult<Option<L1TxEntry>> {
        Ok(self.db.get::<BcastL1TxSchema>(&txid)?)
    }

    fn get_last_txidx(&self) -> DbResult<Option<u64>> {
        Ok(get_last::<BcastL1TxIdSchema>(self.db.as_ref())?.map(|(k, _)| k))
    }

    fn get_txentry_by_idx(&self, idx: u64) -> DbResult<Option<L1TxEntry>> {
        if let Some(id) = self.db.get::<BcastL1TxIdSchema>(&idx)? {
            Ok(self.db.get::<BcastL1TxSchema>(&id)?)
        } else {
            Err(DbError::Other(format!(
                "Entry does not exist for idx {idx:?}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alpen_express_db::errors::DbError;
    use alpen_express_db::traits::{BcastProvider, BcastStore};
    use alpen_express_db::types::L1TxStatus;
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::bitcoin::get_test_bitcoin_txns;
    use alpen_test_utils::get_rocksdb_tmp_instance;
    use bitcoin::hashes::Hash;
    use rockbound::OptimisticTransactionDB as DB;
    use std::sync::Arc;

    fn setup_db() -> Arc<DB> {
        get_rocksdb_tmp_instance().unwrap()
    }

    fn generate_l1_tx_entry() -> (Buf32, L1TxEntry) {
        let txns = get_test_bitcoin_txns();
        let txid = txns[0].compute_txid().as_raw_hash().to_byte_array().into();
        let txentry = L1TxEntry::from_tx(txns[0].clone());
        (txid, txentry)
    }

    #[test]
    fn test_add_tx_new_entry() {
        let db = setup_db();
        let broadcast_db = BroadcastDb::new(db.clone());

        let (txid, txentry) = generate_l1_tx_entry();

        let idx = broadcast_db.add_tx(txid, txentry.clone()).unwrap();

        assert_eq!(idx, 0);

        let stored_entry = broadcast_db.get_txentry_by_idx(idx).unwrap();
        assert_eq!(stored_entry, Some(txentry));
    }

    #[test]
    fn test_add_tx_existing_entry() {
        let db = setup_db();
        let broadcast_db = BroadcastDb::new(db.clone());

        let (txid, txentry) = generate_l1_tx_entry();

        let _ = broadcast_db.add_tx(txid, txentry.clone()).unwrap();

        let result = broadcast_db.add_tx(txid, txentry);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Entry already exists for id"));
        }
    }

    #[test]
    fn test_update_tx() {
        let db = setup_db();
        let broadcast_db = BroadcastDb::new(db.clone());

        let (txid, txentry) = generate_l1_tx_entry();

        // Attempt to update non-existing entry
        let result = broadcast_db.update_tx(txid, txentry.clone());
        assert!(result.is_err());

        // Add and then update the entry
        let _ = broadcast_db.add_tx(txid, txentry.clone()).unwrap();

        let mut updated_txentry = txentry;
        updated_txentry.status = L1TxStatus::Finalized;

        broadcast_db
            .update_tx(txid, updated_txentry.clone())
            .unwrap();

        let stored_entry = broadcast_db.get_txentry(txid).unwrap();
        assert_eq!(stored_entry, Some(updated_txentry));
    }

    #[test]
    fn test_update_tx_by_idx() {
        let db = setup_db();
        let broadcast_db = BroadcastDb::new(db.clone());

        let (txid, txentry) = generate_l1_tx_entry();

        // Attempt to update non-existing index
        let result = broadcast_db.update_tx_by_idx(0, txentry.clone());
        assert!(result.is_err());

        // Add and then update the entry by index
        let idx = broadcast_db.add_tx(txid, txentry.clone()).unwrap();

        let mut updated_txentry = txentry;
        updated_txentry.status = L1TxStatus::Finalized;

        broadcast_db
            .update_tx_by_idx(idx, updated_txentry.clone())
            .unwrap();

        let stored_entry = broadcast_db.get_txentry_by_idx(idx).unwrap();
        assert_eq!(stored_entry, Some(updated_txentry));
    }

    #[test]
    fn test_get_txentry_by_idx() {
        let db = setup_db();
        let broadcast_db = BroadcastDb::new(db.clone());

        // Test non-existing entry
        let result = broadcast_db.get_txentry_by_idx(0);
        assert!(result.is_err());

        let (txid, txentry) = generate_l1_tx_entry();

        let idx = broadcast_db.add_tx(txid, txentry.clone()).unwrap();

        let stored_entry = broadcast_db.get_txentry_by_idx(idx).unwrap();
        assert_eq!(stored_entry, Some(txentry));
    }

    #[test]
    fn test_get_last_txidx() {
        let db = setup_db();
        let broadcast_db = BroadcastDb::new(db.clone());

        let last_txidx = broadcast_db.get_last_txidx().unwrap();
        assert_eq!(last_txidx, None, "There is no last txidx in the beginning");

        let (txid, txentry) = generate_l1_tx_entry();

        let idx = broadcast_db.add_tx(txid, txentry.clone()).unwrap();

        let last_txidx = broadcast_db.get_last_txidx().unwrap();

        assert_eq!(last_txidx, Some(idx));
    }
}
