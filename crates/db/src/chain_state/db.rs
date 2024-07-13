use std::sync::Arc;

use alpen_vertex_state::chain_state::ChainState;
use arbitrary::Arbitrary;
use rockbound::SchemaBatch;
use rockbound::{Schema, DB};

use super::schemas::{ChainStateSchema, WriteBatchSchema};
use crate::errors::*;
use crate::traits::*;

pub struct ChainStateDb {
    db: Arc<DB>,
}

impl ChainStateDb {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    // TODO: maybe move this to common function since this is used in
    // different places
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

    // TODO: maybe move this to common function since this is used in
    // different places
    fn get_first_idx<T>(&self) -> DbResult<Option<u64>>
    where
        T: Schema<Key = u64>,
    {
        let mut iterator = self.db.iter::<T>()?;
        match iterator.next() {
            Some(res) => {
                let (tip, _) = res?.into_tuple();
                Ok(Some(tip))
            }
            None => Ok(None),
        }
    }
}

impl ChainstateProvider for ChainStateDb {
    fn get_earliest_state_idx(&self) -> DbResult<u64> {
        match self.get_first_idx::<ChainStateSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_last_state_idx(&self) -> DbResult<u64> {
        match self.get_last_idx::<ChainStateSchema>()? {
            Some(idx) => Ok(idx),
            None => Err(DbError::NotBootstrapped),
        }
    }

    fn get_writes_at(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::state_op::WriteBatch>> {
        Ok(self.db.get::<WriteBatchSchema>(&idx)?)
    }

    // TODO: define what toplevel means more clearly
    fn get_toplevel_state(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::chain_state::ChainState>> {
        Ok(self.db.get::<ChainStateSchema>(&idx)?)
    }
}

impl ChainstateStore for ChainStateDb {
    fn write_genesis_state(
        &self,
        toplevel: &alpen_vertex_state::chain_state::ChainState,
    ) -> DbResult<()> {
        let genesis_key = 0;
        self.db.put::<ChainStateSchema>(&genesis_key, toplevel)?;
        Ok(())
    }

    fn write_state_update(
        &self,
        idx: u64,
        batch: &alpen_vertex_state::state_op::WriteBatch,
    ) -> DbResult<()> {
        if let Some(_) = self.db.get::<WriteBatchSchema>(&idx)? {
            return Err(DbError::OverwriteStateUpdate(idx));
        }
        let mut write_batch = SchemaBatch::new();
        write_batch.put::<WriteBatchSchema>(&idx, batch)?;
        // TODO: compute new state and write the new state in a SchemaBatch via
        // state_op::apply_write_batch_to_chainstate()
        let updated_state = ChainState::default();
        write_batch.put::<ChainStateSchema>(&idx, &updated_state)?;
        self.db.write_schemas(write_batch)?;
        Ok(())
    }

    fn purge_historical_state_before(&self, before_idx: u64) -> DbResult<()> {
        let first_idx = match self.get_first_idx::<ChainStateSchema>()? {
            Some(idx) => idx,
            None => return Err(DbError::NotBootstrapped),
        };

        if first_idx > before_idx {
            return Err(DbError::Other("test".to_owned()));
        }

        let mut del_batch = SchemaBatch::new();
        for idx in first_idx..before_idx {
            del_batch.delete::<ChainStateSchema>(&idx)?;
        }
        self.db.write_schemas(del_batch)?;
        Ok(())
    }

    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()> {
        let last_idx = match self.get_last_idx::<ChainStateSchema>()? {
            Some(idx) => idx,
            None => return Err(DbError::NotBootstrapped),
        };

        if new_tip_idx > last_idx {
            return Err(DbError::Other("test".to_owned()));
        }

        let mut del_batch = SchemaBatch::new();
        for idx in new_tip_idx + 1..=last_idx {
            del_batch.delete::<ChainStateSchema>(&idx)?;
            del_batch.delete::<ChainStateSchema>(&idx)?;
        }
        self.db.write_schemas(del_batch)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alpen_test_utils::{get_rocksdb_tmp_instance, ArbitraryGenerator};

    use super::*;

    fn setup_db() -> ChainStateDb {
        let db = get_rocksdb_tmp_instance().unwrap();
        ChainStateDb::new(db)
    }
}
