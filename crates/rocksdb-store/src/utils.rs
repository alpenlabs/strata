use std::sync::Arc;

use rockbound::{OptimisticTransactionDB as DB, Schema, SchemaDBOperationsExt};

use alpen_express_db::DbResult;

pub fn get_last_idx<T>(db: Arc<DB>) -> DbResult<Option<u64>>
where
    T: Schema<Key = u64>,
{
    let mut iterator = db.iter::<T>()?;
    iterator.seek_to_last();
    match iterator.rev().next() {
        Some(res) => {
            let (tip, _) = res?.into_tuple();
            Ok(Some(tip))
        }
        None => Ok(None),
    }
}

pub fn get_first_idx<T>(db: Arc<DB>) -> DbResult<Option<u64>>
where
    T: Schema<Key = u64>,
{
    let mut iterator = db.iter::<T>()?;
    match iterator.next() {
        Some(res) => {
            let (tip, _) = res?.into_tuple();
            Ok(Some(tip))
        }
        None => Ok(None),
    }
}
