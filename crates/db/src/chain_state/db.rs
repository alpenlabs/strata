use std::sync::Arc;

use anyhow::Chain;
use rockbound::SchemaBatch;
use rockbound::{Schema, DB};

use alpen_vertex_state::operation::*;

use alpen_vertex_state::chain_state::ChainState;
use alpen_vertex_state::state_op;
use alpen_vertex_state::state_op::WriteBatch;


use crate::traits::*;
use crate::errors::*;

use super::schemas::{ChainStateSchema, ChainWriteBatchSchema};

pub struct ChainStateDB {
    db: Arc<DB>
}

impl ChainStateDB {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    fn find_last_write_batch(&self) -> DbResult<Option<u64>> {
        let iterator = self.db.iter::<ChainStateSchema>()?;
        iterator.seek_to_last();

        if iterator.valid() {
            let (tip, _) = iterator.into_tuple();
            Ok(Some(tip))
        } else {
            Err("chainstatedb: Genesis not written")
        }
    }

}

impl ChainstateStore for ChainStateDB {
    fn write_genesis_state(&self, toplevel: &ChainState) -> DbResult<()> {
        self.db.put::<ChainState>(&0, &toplevel.clone())?;
        Ok(())
    }

    fn write_state_update(&self, idx: u64, batch: &WriteBatch) -> DbResult<()> {
        let last_idx = self.find_last_write_batch()??;
        if idx <= last_idx {
            return Err(DbError::OooInsert("chainstate", idx))
        }

        let mut toplevel = self.db.get::<ChainStateSchema>(&last_idx)??;

        let new_state = state_op::apply_write_batch_to_chainstate(toplevel, batch);
        let mut batch = SchemaBatch::new();
        batch.put::<ChainStateSchema>(&idx,&new_state);
        batch.put::<ChainWriteBatchSchema>(&idx, &batch.clone());
        self.db.write_schemas(batch)?;


        Ok(())
    }

    fn purge_historical_state_before(&self, before_idx: u64) -> DbResult<()> {
        let top_level = self.db.get::<ChainStateSchema>(&before_idx)?;
        if let None = top_level {
            return Err(DbError::UnknownIdx(before_idx));
        }
        let last_idx = self.find_last_write_batch();
        if before_idx >= last_idx {
            return Err(DbError::PurgeTooRecent);
        }

        // let iterator = self.db;
        let mut cs_iterator = self.db.iter::<ChainStateSchema>()?;
        cs_iterator.seek(&before_idx);

        let mut batch = SchemaBatch::new();
        while let Some(cs) = cs_iterator.next() {
            let (cs_key,_) = *cs?;
            if cs_key < before_idx {
                batch.delete::<ChainStateSchema>(cs_key)?;
                batch.delete::<ChainWriteBatchSchema>(cs_key)?;
            }
        }

        // let mut cs_batch_iterator = self.db.iter::<ChainWriteBatchSchema>()?;
        // while let Some(cs) = cs_batch_iterator.next() {
        //     let (cs_key,_) = *cs?;
        //     if cs_key < before_idx {
        //         batch.delete::<ChainWriteBatchSchema>(cs_key)?;
        //     }
        // }

        //TODO: finalize batch
        
        Ok(())
    }

    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()> {
        // the key should already be in the DB
        let top_level = self.db.get::<ChainStateSchema>(&before_idx)?;
        if let None = top_level {
            return Err(DbError::UnknownIdx(before_idx));
        }

        let last_idx = self.find_last_write_batch();
        if new_tip_idx > last_idx {
            return Err(DbError::RevertAboveCurrent);
        }
        
        let mut batch = SchemaBatch::new();
        let mut cs_iterator = self.db.iter::<ChainStateSchema>()?.rev();
        while let Some(cs) = cs_batch_iterator.next() {
            let (cs_key, _) = *cs?;
            if cs_key <= new_tip_idx {
                break;
            }
            batch.delete::<ChainStateSchema>(cs_key);
            batch.delete::<ChainWriteBatchSchema>(cs_key);
            // TODO: we have to check if the data is present in the DB or not before deleting 
        }
        // TODO: Apply batch and check we are where we should be 

        Ok(())
    }
}

impl ChainstateProvider for ChainStateDB {
    fn get_last_state_idx(&self) -> DbResult<u64> {
        let last_idx = self.find_last_write_batch()?.expect("CS not written");
        Ok(last_idx)
    }

    fn get_earliest_state_idx(&self) -> DbResult<u64> {
        let cs_iterator = self.db.iter::<ChainStateSchema>()?;
        if let Some(cs) = cs_iterator.next() {
            let (key,_) = cs?;
            Ok(key)
        }
        //TODO: identify the error
        Err(DbError::Unimplemented)
    }

    fn get_writes_at(&self, idx: u64) -> DbResult<Option<WriteBatch>> {
        Ok(self.db.get::<ChainWriteBatchSchema>(&idx)?)
    }

    fn get_toplevel_state(&self, idx: u64) -> DbResult<Option<ChainState>> {
        Ok(self.db.get::<ChainState>(&idx)?)
    }
}

// TODO: Test
