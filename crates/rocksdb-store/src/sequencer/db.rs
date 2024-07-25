use std::sync::Arc;

use alpen_express_primitives::buf::Buf32;
use bitcoin::{hashes::Hash, Transaction};
use rockbound::{OptimisticTransactionDB as DB, Schema, SchemaBatch, SchemaDBOperationsExt};

use alpen_express_db::{
    errors::DbError,
    traits::{SeqDataProvider, SeqDataStore, SequencerDatabase},
    types::{BlobEntry, TxEntry},
    DbResult,
};

use super::schemas::{SeqBIdRevTxnIdxSchema, SeqBlobIdSchema, SeqBlobSchema, SeqL1TxnSchema};

pub struct SeqDb {
    db: Arc<DB>,
}

impl SeqDb {
    /// Wraps an existing database handle.
    ///
    /// Assumes it was opened with column families as defined in `STORE_COLUMN_FAMILIES`.
    // FIXME Make it better/generic.
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    fn get_last_idx<T>(&self) -> DbResult<Option<u64>>
    where
        T: Schema<Key = u64>,
    {
        let mut iterator = self.db.iter::<T>()?;
        iterator.seek_to_last();
        match iterator.rev().next() {
            Some(res) => {
                let (tip, _) = res?.into_tuple();
                Ok(Some(tip))
            }
            None => Ok(None),
        }
    }
}

impl SeqDataStore for SeqDb {
    fn put_blob(&self, blob_hash: Buf32, blob: BlobEntry) -> DbResult<u64> {
        if self.db.get::<SeqBlobSchema>(&blob_hash)?.is_some() {
            return Err(DbError::Other(format!(
                "Entry already exists for blobid {blob_hash:?}"
            )));
        }
        // TODO: wrap these in a db transaction
        let idx = self
            .get_last_idx::<SeqBlobIdSchema>()?
            .map(|x| x + 1)
            .unwrap_or(0);

        let mut batch = SchemaBatch::new();

        // Atomically add the entries
        batch.put::<SeqBlobIdSchema>(&idx, &blob_hash)?;
        batch.put::<SeqBlobSchema>(&blob_hash, &blob)?;

        self.db.write_schemas(batch)?;

        Ok(idx)
    }

    fn put_commit_reveal_txs(
        &self,
        commit_tx: Transaction,
        reveal_tx: Transaction,
    ) -> DbResult<(Buf32, Buf32)> {
        let commit_txid = Buf32::from(*commit_tx.compute_txid().as_raw_hash().as_byte_array());
        let reveal_txid = Buf32::from(*reveal_tx.compute_txid().as_raw_hash().as_byte_array());
        todo!();
        Ok((commit_txid, reveal_txid))
    }

    fn update_blob_by_idx(&self, blobidx: u64, blobentry: BlobEntry) -> DbResult<()> {
        todo!()
    }
}

impl SeqDataProvider for SeqDb {
    fn get_blob_by_id(&self, id: Buf32) -> DbResult<Option<BlobEntry>> {
        Ok(self.db.get::<SeqBlobSchema>(&id)?)
    }

    fn get_last_blob_idx(&self) -> DbResult<Option<u64>> {
        self.get_last_idx::<SeqBlobIdSchema>()
    }

    fn get_l1_tx(&self, txid: Buf32) -> DbResult<Option<Vec<u8>>> {
        todo!()
    }

    fn get_blob_by_idx(&self, blobidx: u64) -> DbResult<Option<crate::types::BlobEntry>> {
        todo!()
    }
}

pub struct SequencerDB<D> {
    db: Arc<D>,
}

impl<D> SequencerDB<D> {
    pub fn new(db: Arc<D>) -> Self {
        Self { db }
    }
}

impl<D: SeqDataStore + SeqDataProvider> SequencerDatabase for SequencerDB<D> {
    type SeqStore = D;
    type SeqProv = D;

    fn sequencer_store(&self) -> &Arc<Self::SeqStore> {
        &self.db
    }

    fn sequencer_provider(&self) -> &Arc<Self::SeqProv> {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::DbError;
    use crate::traits::{SeqDataProvider, SeqDataStore};
    use crate::types::TxEntry;
    use crate::types::TxnStatusEntry;
    use alpen_express_db::errors::DbError;
    use alpen_express_db::traits::{SeqDataProvider, SeqDataStore};
    use alpen_express_db::types::TxnStatusEntry;
    use alpen_express_primitives::buf::Buf32;
    use alpen_test_utils::bitcoin::get_test_bitcoin_txns;
    use alpen_test_utils::get_rocksdb_tmp_instance;
    use rockbound::OptimisticTransactionDB as DB;
    use std::sync::Arc;
    use test;

    fn setup_db() -> Arc<DB> {
        get_rocksdb_tmp_instance().unwrap()
    }

    fn get_commit_reveal_txns() -> (TxEntry, TxEntry) {
        let txns = get_test_bitcoin_txns();

        // NOTE that actually the commit reveal should be parent-child, but these are not.
        // This shouldn't matter here though.
        let commit_txn = TxEntry::from_txn(&txns[0]);
        let reveal_txn = TxEntry::from_txn(&txns[1]);
        (commit_txn, reveal_txn)
    }

    #[test]
    fn test_put_blob_new_entry() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];

        let result = seq_db.put_blob(blob_hash, blob.clone());

        assert!(result.is_ok());
        let idx = result.unwrap();
        assert_eq!(idx, 0);

        //Also check appropriate mapping is created
        assert_eq!(
            seq_db.get_blobid_for_blob_idx(idx).unwrap(),
            Some(blob_hash)
        );

        let stored_blob = seq_db.get_blob_by_id(blob_hash).unwrap();
        assert_eq!(stored_blob, Some(blob));
    }

    #[test]
    fn test_put_blob_existing_entry() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];

        seq_db.put_blob(blob_hash, blob.clone()).unwrap();
        let result = seq_db.put_blob(blob_hash, blob);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Entry already exists for blobid"));
        }
    }

    #[test]
    fn test_put_commit_reveal_txns_existing_blobid() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];
        seq_db.put_blob(blob_hash, blob).unwrap();

        let (commit_txn, reveal_txn) = get_commit_reveal_txns();

        let result =
            seq_db.put_commit_reveal_txns(blob_hash, commit_txn.clone(), reveal_txn.clone());

        assert!(result.is_ok());
        let reveal_idx = result.unwrap();
        assert_eq!(reveal_idx, 1);

        let stored_commit_txn = seq_db.get_l1_txn(0).unwrap();
        assert_eq!(stored_commit_txn, Some(commit_txn));

        let stored_reveal_txn = seq_db.get_l1_txn(1).unwrap();
        assert_eq!(stored_reveal_txn, Some(reveal_txn));

        // Check if blobid -> txidx mapping is created
        assert_eq!(
            seq_db.get_reveal_txidx_for_blob(blob_hash).unwrap(),
            Some(reveal_idx)
        );
    }

    #[test]
    fn test_put_commit_reveal_txns_nonexistent_blobid() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([1u8; 32]);
        let (commit_txn, reveal_txn) = get_commit_reveal_txns();

        let result = seq_db.put_commit_reveal_txns(blob_hash, commit_txn, reveal_txn);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Inexistent blobid"));
        }
    }

    #[test]
    fn test_update_txn_existing() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let txn_idx = 1;
        let (txn, new_txn) = get_commit_reveal_txns();
        seq_db.db.put::<SeqL1TxnSchema>(&txn_idx, &txn).unwrap();

        let result = seq_db.update_txn(txn_idx, new_txn.clone());

        assert!(result.is_ok());

        let updated_txn = seq_db.get_l1_txn(txn_idx).unwrap();
        assert_eq!(updated_txn, Some(new_txn));
    }

    #[test]
    fn test_update_txn_nonexistent() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let txn_idx = 1;
        let (new_txn, _) = get_commit_reveal_txns();

        let result = seq_db.update_txn(txn_idx, new_txn);

        assert!(result.is_err());
        if let Err(DbError::Other(err)) = result {
            assert!(err.contains("Inexistent txn idx"));
        }
    }

    #[test]
    fn test_get_l1_txn_existing() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let txn_idx = 1;
        let (txn, _) = get_commit_reveal_txns();
        seq_db.db.put::<SeqL1TxnSchema>(&txn_idx, &txn).unwrap();

        let result = seq_db.get_l1_txn(txn_idx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(txn));
    }

    #[test]
    fn test_get_l1_txn_nonexistent() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let txn_idx = 1;
        let result = seq_db.get_l1_txn(txn_idx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_get_blob_by_id_existing() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];
        seq_db.put_blob(blob_hash, blob.clone()).unwrap();

        let result = seq_db.get_blob_by_id(blob_hash);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(blob));
    }

    #[test]
    fn test_get_blob_by_id_nonexistent() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let result = seq_db.get_blob_by_id(blob_hash);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_get_last_blob_idx_empty_db() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let result = seq_db.get_last_blob_idx();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_get_last_blob_idx_nonempty_db() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];
        seq_db.put_blob(blob_hash, blob).unwrap();

        let result = seq_db.get_last_blob_idx();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(0));
    }

    #[test]
    fn test_get_reveal_txidx_for_blob_existing() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];
        seq_db.put_blob(blob_hash, blob).unwrap();

        let (commit_txn, reveal_txn) = get_commit_reveal_txns();
        seq_db
            .put_commit_reveal_txns(blob_hash, commit_txn, reveal_txn)
            .unwrap();

        let result = seq_db.get_reveal_txidx_for_blob(blob_hash);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(1));
    }

    #[test]
    fn test_get_reveal_txidx_for_blob_nonexistent() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let result = seq_db.get_reveal_txidx_for_blob(blob_hash);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_get_blobid_for_blob_idx_existing() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];
        let idx = seq_db.put_blob(blob_hash, blob).unwrap();

        let result = seq_db.get_blobid_for_blob_idx(idx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(blob_hash));
    }

    #[test]
    fn test_get_blobid_for_blob_idx_nonexistent() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let result = seq_db.get_blobid_for_blob_idx(1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_get_last_txn_idx_none() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let result = seq_db.get_last_txn_idx().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_last_txn_idx_some() {
        let db = setup_db();
        let seq_db = SeqDb::new(db.clone());

        let blob_hash = Buf32::from([0u8; 32]);
        let blob = vec![1, 2, 3];
        seq_db.put_blob(blob_hash, blob).unwrap();

        let (ctxn, rtxn) = get_commit_reveal_txns();
        let _ = seq_db.put_commit_reveal_txns(blob_hash, ctxn, rtxn);

        let result = seq_db.get_last_txn_idx().unwrap();
        assert_eq!(result, Some(1));
    }
}
