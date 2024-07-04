use std::sync::Arc;

use alpen_vertex_primitives::{buf::Buf32, l1::TxnWithStatus};
use rockbound::{Schema, DB};

use crate::{
    traits::{SeqDataProvider, SeqDataStore},
    DbResult,
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
        // TODO: complete this
        Ok(0)
    }

    /// Store commit reveal txns associated with the blobid.
    fn put_commit_reveal_txns(
        &self,
        blob_id: Buf32,
        commit_txn: TxnWithStatus,
        reveal_txn: TxnWithStatus,
    ) -> DbResult<u64> {
        // TODO: COMPLETE this
        Ok(1)
    }

    fn update_txn(&self, txid: Buf32, txn: TxnWithStatus) -> DbResult<()> {
        todo!()
    }
}

impl SeqDataProvider for SeqDb {
    fn get_l1_txn(&self, idx: u64) -> DbResult<Option<TxnWithStatus>> {
        todo!()
    }

    fn get_blob_by_id(&self, id: Buf32) -> DbResult<Option<Vec<u8>>> {
        todo!()
    }

    fn get_last_txn_idx(&self) -> DbResult<Option<u64>> {
        todo!()
    }

    fn get_last_blob_idx(&self) -> DbResult<Option<u64>> {
        todo!()
    }

    fn get_txidx_for_blob(&self, blobid: Buf32) -> DbResult<Option<u64>> {
        todo!()
    }

    fn get_blobid_for_blob_idx(&self, blobidx: u64) -> DbResult<Buf32> {
        todo!()
    }
}
