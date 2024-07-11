use std::sync::Arc;

use anyhow::Chain;
use rockbound::{Schema, DB};

use alpen_vertex_state::operation::*;

use super::schemas::{ChainStateSchema, WriteBatchSchema};
use crate::errors::*;
use crate::traits::*;

pub struct ChainStateDb {
    db: Arc<DB>,
}

impl ChainStateDb {}

impl ChainstateProvider for ChainStateDb {
    fn get_earliest_state_idx(&self) -> DbResult<u64> {
        unimplemented!()
    }

    fn get_last_state_idx(&self) -> DbResult<u64> {
        unimplemented!()
    }

    fn get_toplevel_state(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::chain_state::ChainState>> {
        unimplemented!()
    }

    fn get_writes_at(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::state_op::WriteBatch>> {
        unimplemented!()
    }
}

impl ChainstateStore for ChainStateDb {
    fn purge_historical_state_before(&self, before_idx: u64) -> DbResult<()> {
        unimplemented!()
    }

    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()> {
        unimplemented!()
    }

    fn write_genesis_state(
        &self,
        toplevel: &alpen_vertex_state::chain_state::ChainState,
    ) -> DbResult<()> {
        unimplemented!()
    }

    fn write_state_update(
        &self,
        idx: u64,
        batch: &alpen_vertex_state::state_op::WriteBatch,
    ) -> DbResult<()> {
        unimplemented!()
    }
}
