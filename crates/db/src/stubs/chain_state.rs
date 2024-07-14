use std::collections::*;

use parking_lot::Mutex;
use tracing::*;

use alpen_vertex_state::chain_state::ChainState;
use alpen_vertex_state::state_op;
use alpen_vertex_state::state_op::WriteBatch;

use crate::errors::*;
use crate::traits::*;

struct InnerState {
    write_batches: BTreeMap<u64, WriteBatch>,
    toplevels: BTreeMap<u64, ChainState>,
}

impl InnerState {
    pub fn new() -> Self {
        Self {
            write_batches: BTreeMap::new(),
            toplevels: BTreeMap::new(),
        }
    }

    fn find_last_write_batch(&self) -> u64 {
        self.toplevels
            .last_key_value()
            .map(|(k, _)| *k)
            .expect("chainstatedb: genesis not written")
    }
}

pub struct StubChainstateDb {
    state: Mutex<InnerState>,
}

impl StubChainstateDb {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(InnerState::new()),
        }
    }
}

impl ChainstateStore for StubChainstateDb {
    fn write_genesis_state(&self, toplevel: &ChainState) -> DbResult<()> {
        let mut st = self.state.lock();
        st.toplevels.insert(0, toplevel.clone());
        Ok(())
    }

    fn write_state_update(&self, idx: u64, batch: &WriteBatch) -> DbResult<()> {
        let mut st = self.state.lock();

        let last_idx = st.find_last_write_batch();
        if idx <= last_idx {
            return Err(DbError::OooInsert("chainstate", idx));
        }

        let toplevel = st
            .toplevels
            .get(&last_idx)
            .cloned()
            .expect("chainstatedb: nonsense");

        // Compute new state and insert things.
        let new_state = state_op::apply_write_batch_to_chainstate(toplevel, batch);
        st.toplevels.insert(idx, new_state);
        st.write_batches.insert(idx, batch.clone());

        Ok(())
    }

    fn purge_historical_state_before(&self, before_idx: u64) -> DbResult<()> {
        let mut st = self.state.lock();

        if !st.toplevels.contains_key(&before_idx) {
            return Err(DbError::UnknownIdx(before_idx));
        }

        let last_idx = st.find_last_write_batch();
        if before_idx >= last_idx {
            return Err(DbError::PurgeTooRecent);
        }

        // Remove from the two tables.  This does have to touch every state in
        // the table but it's fine because this will never be used in production.
        let states_removed = st.toplevels.extract_if(|idx, _| *idx < before_idx).count();
        let writes_removed = st
            .write_batches
            .extract_if(|idx, _| *idx < before_idx)
            .count();

        // In case it screws up we should remember it.
        trace!(%states_removed, %writes_removed, %before_idx, "purge_historical_state_before");

        Ok(())
    }

    fn rollback_writes_to(&self, new_tip_idx: u64) -> DbResult<()> {
        let mut st = self.state.lock();

        if !st.toplevels.contains_key(&new_tip_idx) {
            return Err(DbError::UnknownIdx(new_tip_idx));
        }

        let last_idx = st.find_last_write_batch();
        if new_tip_idx > last_idx {
            return Err(DbError::RevertAboveCurrent(new_tip_idx, last_idx));
        }

        // We take a more sensitive approach to this since we don't want to have to
        let to_remove = st
            .toplevels
            .iter()
            .rev()
            .take_while(|(idx, _)| **idx > new_tip_idx)
            .map(|(idx, _)| *idx)
            .collect::<Vec<_>>();

        for rem in to_remove {
            assert!(st.toplevels.remove(&rem).is_some());
            assert!(st.write_batches.remove(&rem).is_some());
        }

        // Check that we're where we expect to be.
        let (k, _) = st.toplevels.last_key_value().unwrap();
        assert_eq!(*k, new_tip_idx);

        Ok(())
    }
}

impl ChainstateProvider for StubChainstateDb {
    fn get_last_state_idx(&self) -> DbResult<u64> {
        let st = self.state.lock();
        Ok(st.find_last_write_batch())
    }

    fn get_earliest_state_idx(&self) -> DbResult<u64> {
        let st = self.state.lock();
        let idx = st
            .toplevels
            .first_key_value()
            .map(|(k, _)| *k)
            .expect("chainstatedb: genesis not written");
        Ok(idx)
    }

    fn get_writes_at(&self, idx: u64) -> DbResult<Option<WriteBatch>> {
        let st = self.state.lock();
        Ok(st.write_batches.get(&idx).cloned())
    }

    fn get_toplevel_state(
        &self,
        idx: u64,
    ) -> DbResult<Option<alpen_vertex_state::chain_state::ChainState>> {
        let st = self.state.lock();
        Ok(st.toplevels.get(&idx).cloned())
    }
}
