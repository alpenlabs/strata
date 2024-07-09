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
        let mut batch = SchemaBatch::new(); 
        batch.put::<ChainState>(&0, &toplevel.clone())?;
        self.db.write_schemas(batch)?;
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
        let mut iterator = self.db.iter::<ChainStateSchema>()?;
        iterator.seek(&before_idx);

        let mut batch = SchemaBatch::new();
        while let Some(cs_iter) = iterator.next() {
            let (cur_state_key,_) = *cs_iter?;

            batch.delete::<ChainStateSchema>(&cur_chain_state.key)?;
        }
        


        
        todo!()
    }

    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()> {
        todo!()
    }
}


