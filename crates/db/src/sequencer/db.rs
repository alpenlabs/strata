use std::sync::Arc;

use alpen_vertex_primitives::{buf::Buf32, l1::TxnWithStatus};
use rockbound::{Schema, SchemaBatch, DB};

use crate::{
    errors::DbError,
    traits::{SeqDataProvider, SeqDataStore},
    DbResult,
};

use super::schemas::{
    SequencerBlobIdSchema, SequencerBlobIdTxnIdxSchema, SequencerBlobSchema, SequencerL1TxnSchema,
};

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
    fn put_blob(&self, blob_hash: Buf32, blob: Vec<u8>) -> DbResult<u64> {
        if self.db.get::<SequencerBlobSchema>(&blob_hash)?.is_some() {
            return Err(DbError::Other(format!(
                "Entry already exists for blobid {blob_hash:?}"
            )));
        }
        let last_idx = self.get_last_idx::<SequencerBlobIdSchema>()?.unwrap_or(0);
        let idx = last_idx + 1;

        let mut batch = SchemaBatch::new();

        // Atomically add the entries
        batch.put::<SequencerBlobIdSchema>(&idx, &blob_hash)?;
        batch.put::<SequencerBlobSchema>(&blob_hash, &blob)?;

        self.db.write_schemas(batch)?;

        Ok(idx)
    }

    fn put_commit_reveal_txns(
        &self,
        blobid: Buf32,
        commit_txn: TxnWithStatus,
        reveal_txn: TxnWithStatus,
    ) -> DbResult<u64> {
        if self.db.get::<SequencerBlobSchema>(&blobid)?.is_none() {
            return Err(DbError::Other(format!(
                "Inexistent blobid {blobid:?} while storing commit reveal txn"
            )));
        }

        let last_reveal_idx = self.get_last_idx::<SequencerL1TxnSchema>()?.unwrap_or(0);
        let commit_idx = last_reveal_idx + 1;
        let reveal_idx = commit_idx + 1;

        let mut batch = SchemaBatch::new();

        // Atomically add entries
        batch.put::<SequencerL1TxnSchema>(&commit_idx, &commit_txn)?;
        batch.put::<SequencerL1TxnSchema>(&reveal_idx, &reveal_txn)?;
        batch.put::<SequencerBlobIdTxnIdxSchema>(&blobid, &reveal_idx)?;

        self.db.write_schemas(batch)?;

        Ok(reveal_idx)
    }

    fn update_txn(&self, txidx: u64, txn: TxnWithStatus) -> DbResult<()> {
        if self.db.get::<SequencerL1TxnSchema>(&txidx)?.is_none() {
            return Err(DbError::Other(format!(
                "Inexistent txn idx {txidx:?} while updating txn"
            )));
        }
        self.db.put::<SequencerL1TxnSchema>(&txidx, &txn)?;
        Ok(())
    }
}

impl SeqDataProvider for SeqDb {
    fn get_l1_txn(&self, idx: u64) -> DbResult<Option<TxnWithStatus>> {
        Ok(self.db.get::<SequencerL1TxnSchema>(&idx)?)
    }

    fn get_blob_by_id(&self, id: Buf32) -> DbResult<Option<Vec<u8>>> {
        Ok(self.db.get::<SequencerBlobSchema>(&id)?)
    }

    fn get_last_blob_idx(&self) -> DbResult<Option<u64>> {
        self.get_last_idx::<SequencerBlobIdSchema>()
    }

    fn get_txidx_for_blob(&self, blobid: Buf32) -> DbResult<Option<u64>> {
        Ok(self.db.get::<SequencerBlobIdTxnIdxSchema>(&blobid)?)
    }

    fn get_blobid_for_blob_idx(&self, blobidx: u64) -> DbResult<Option<Buf32>> {
        Ok(self.db.get::<SequencerBlobIdSchema>(&blobidx)?)
    }

    fn get_last_txn_idx(&self) -> DbResult<Option<u64>> {
        self.get_last_idx::<SequencerL1TxnSchema>()
    }
}
