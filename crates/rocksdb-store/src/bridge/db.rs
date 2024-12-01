use std::sync::Arc;

use rockbound::{OptimisticTransactionDB as DB, SchemaDBOperationsExt, TransactionRetry};
use strata_db::{
    entities::bridge_tx_state::BridgeTxState,
    errors::DbError,
    traits::{BridgeDutyDatabase, BridgeDutyIndexDatabase, BridgeTxDatabase},
    DbResult,
};
use strata_primitives::buf::Buf32;
use strata_state::bridge_duties::BridgeDutyStatus;

use super::schemas::{
    BridgeDutyCheckpointSchema, BridgeDutyStatusSchema, BridgeDutyTxidSchema, BridgeTxStateSchema,
    BridgeTxStateTxidSchema,
};
use crate::{sequence::get_next_id, DbOpsConfig};

pub struct BridgeTxRocksDb {
    db: Arc<DB>,
    ops: DbOpsConfig,
}

impl BridgeTxRocksDb {
    pub fn new(db: Arc<DB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl BridgeTxDatabase for BridgeTxRocksDb {
    fn put_tx_state(&self, txid: Buf32, tx_state: BridgeTxState) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                // insert new id if the txid is new
                if txn.get::<BridgeTxStateSchema>(&txid)?.is_none() {
                    let idx = get_next_id::<BridgeTxStateTxidSchema, DB>(txn)?;

                    txn.put::<BridgeTxStateTxidSchema>(&idx, &txid)?;
                }

                txn.put::<BridgeTxStateSchema>(&txid, &tx_state)?;

                Ok::<(), DbError>(())
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }

    fn delete_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
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

    fn get_tx_state(&self, txid: Buf32) -> DbResult<Option<BridgeTxState>> {
        Ok(self.db.get::<BridgeTxStateSchema>(&txid)?)
    }
}

pub struct BridgeDutyRocksDb {
    db: Arc<DB>,
    ops: DbOpsConfig,
}

impl BridgeDutyRocksDb {
    pub fn new(db: Arc<DB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl BridgeDutyDatabase for BridgeDutyRocksDb {
    fn get_status(&self, txid: Buf32) -> DbResult<Option<BridgeDutyStatus>> {
        Ok(self.db.get::<BridgeDutyStatusSchema>(&txid)?)
    }

    fn put_duty_status(&self, txid: Buf32, status: BridgeDutyStatus) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                if txn.get::<BridgeDutyStatusSchema>(&txid)?.is_none() {
                    let idx = get_next_id::<BridgeDutyTxidSchema, DB>(txn)?;

                    txn.put::<BridgeDutyTxidSchema>(&idx, &txid)?;
                }

                txn.put::<BridgeDutyStatusSchema>(&txid, &status)?;

                Ok::<(), DbError>(())
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }

    fn delete_duty(&self, txid: Buf32) -> DbResult<Option<BridgeDutyStatus>> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                if let Some(status) = txn.get::<BridgeDutyStatusSchema>(&txid)? {
                    txn.delete::<BridgeDutyStatusSchema>(&txid)?;
                    return Ok::<Option<BridgeDutyStatus>, DbError>(Some(status));
                }

                Ok(None)
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }
}

pub struct BridgeDutyIndexRocksDb {
    db: Arc<DB>,
    ops: DbOpsConfig,
}

impl BridgeDutyIndexRocksDb {
    pub fn new(db: Arc<DB>, ops: DbOpsConfig) -> Self {
        Self { db, ops }
    }
}

impl BridgeDutyIndexDatabase for BridgeDutyIndexRocksDb {
    fn get_index(&self) -> DbResult<Option<u64>> {
        Ok(self.db.get::<BridgeDutyCheckpointSchema>(&0)?)
    }

    fn set_index(&self, checkpoint: u64) -> DbResult<()> {
        self.db
            .with_optimistic_txn(TransactionRetry::Count(self.ops.retry_count), |txn| {
                txn.put::<BridgeDutyCheckpointSchema>(&0, &checkpoint)?;

                Ok::<(), DbError>(())
            })
            .map_err(|e: rockbound::TransactionError<_>| DbError::TransactionError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary, Unstructured};
    use strata_db::traits::BridgeTxDatabase;
    use strata_primitives::l1::BitcoinTxid;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;
    use crate::test_utils::get_rocksdb_tmp_instance;

    #[test]
    fn test_bridge_tx_state_db() {
        let db = setup_tx_db();

        let raw_bytes = vec![0u8; 1024];
        let mut u = Unstructured::new(&raw_bytes);

        let bridge_tx_state = BridgeTxState::arbitrary(&mut u).unwrap();

        let txid = bridge_tx_state.compute_txid().into();

        // Test insert
        let result = db.put_tx_state(txid, bridge_tx_state.clone());
        assert!(
            result.is_ok(),
            "should be able to add collected sigs but got: {}",
            result.err().unwrap()
        );

        // Test read
        let stored_entry = db.get_tx_state(txid);
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
        let result = db.put_tx_state(txid, new_state.clone());
        assert!(
            result.is_ok(),
            "should be able to update existing data at a given Txid but got: {}",
            result.err().unwrap()
        );

        let stored_entry = db.get_tx_state(txid);
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
        let evicted_entry = db.delete_tx_state(txid).unwrap();
        assert!(
            evicted_entry.is_some_and(|entry| entry == stored_entry.unwrap()),
            "stored entry should be returned after being evicted"
        );

        let re_evicted_entry = db.delete_tx_state(txid).unwrap();
        assert!(
            re_evicted_entry.is_none(),
            "evicting an already evicted entry should return None"
        );

        let stored_entry = db.get_tx_state(txid).unwrap();
        assert!(
            stored_entry.is_none(),
            "stored entry should not be present after eviction"
        );
    }

    fn setup_tx_db() -> BridgeTxRocksDb {
        let (db, config) = get_rocksdb_tmp_instance().unwrap();

        BridgeTxRocksDb::new(db, config)
    }

    #[test]
    fn test_bridge_duty_status_db() {
        let db = setup_duty_db();

        let mut arb = ArbitraryGenerator::new();

        let duty_status: BridgeDutyStatus = arb.generate();
        let txid: BitcoinTxid = arb.generate();
        let txid: Buf32 = txid.inner().into();

        // Test insert
        let result = db.put_duty_status(txid, duty_status.clone());
        assert!(
            result.is_ok(),
            "should be able to add a duty but got: {}",
            result.err().unwrap()
        );

        // Test read
        let stored_entry = db.get_status(txid);
        assert!(
            stored_entry.is_ok(),
            "should be able to access stored entry but got: {}",
            stored_entry.err().unwrap()
        );

        let stored_entry = stored_entry.unwrap();
        assert_eq!(
            stored_entry,
            Some(duty_status),
            "stored duty entry must be in the initial state"
        );

        // Test update
        let new_duty_status: BridgeDutyStatus = arb.generate();
        let result = db.put_duty_status(txid, new_duty_status.clone());
        assert!(
            result.is_ok(),
            "should be able to update existing data at a given Txid but got: {}",
            result.err().unwrap()
        );

        let stored_entry = db.get_status(txid);
        assert!(
            stored_entry.is_ok(),
            "should be able to access updated stored entry but got: {}",
            stored_entry.err().unwrap()
        );

        let stored_entry = stored_entry.unwrap();
        assert_eq!(
            stored_entry,
            Some(new_duty_status),
            "stored entity should match the updated entity being stored"
        );

        // Test delete
        let evicted_entry = db.delete_duty(txid).unwrap();
        assert!(
            evicted_entry.is_some_and(|entry| entry == stored_entry.unwrap()),
            "stored entry should be returned after being evicted"
        );

        let re_evicted_entry = db.delete_duty(txid).unwrap();
        assert!(
            re_evicted_entry.is_none(),
            "evicting an already evicted entry should return None"
        );

        let stored_entry = db.get_status(txid).unwrap();
        assert!(
            stored_entry.is_none(),
            "stored entry should not be present after eviction"
        );
    }

    fn setup_duty_db() -> BridgeDutyRocksDb {
        let (db, config) = get_rocksdb_tmp_instance().unwrap();

        BridgeDutyRocksDb::new(db, config)
    }

    #[test]
    fn test_bridge_duty_index_db() {
        let db = setup_bridge_duty_index_db();

        let mut arb = ArbitraryGenerator::new();

        let checkpoint: u64 = arb.generate();

        // Test get with no checkpoint
        let result = db.set_index(checkpoint);
        assert!(
            result.is_ok(),
            "should be able to set checkpoint but got: {}",
            result.err().unwrap()
        );

        // Test read
        let stored_entry = db.get_index();
        assert!(
            stored_entry.is_ok(),
            "should be able to access stored entry but got: {}",
            stored_entry.err().unwrap()
        );

        let stored_entry = stored_entry.unwrap();
        assert_eq!(
            stored_entry,
            Some(checkpoint),
            "stored entity should match the entity being stored"
        );

        // Test update
        let new_checkpoint: u64 = arb.generate();
        let result = db.set_index(new_checkpoint);
        assert!(
            result.is_ok(),
            "should be able to update existing data at a given Txid but got: {}",
            result.err().unwrap()
        );

        let stored_entry = db.get_index();
        assert!(
            stored_entry.is_ok(),
            "should be able to access updated stored entry but got: {}",
            stored_entry.err().unwrap()
        );

        let stored_entry = stored_entry.unwrap();
        assert_eq!(
            stored_entry,
            Some(new_checkpoint),
            "stored entity should match the updated entity being stored"
        );
    }

    fn setup_bridge_duty_index_db() -> BridgeDutyIndexRocksDb {
        let (db, config) = get_rocksdb_tmp_instance().unwrap();

        BridgeDutyIndexRocksDb::new(db, config)
    }
}
