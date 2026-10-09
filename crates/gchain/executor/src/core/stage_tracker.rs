//! Where each stage stands, kept in step with the store.
//!
//! This is the other half of the pipeline's position: the commit log says
//! what's committed, this says how far each stage has committed it.  It
//! knows nothing of the committed path itself; that a stage's node is on it
//! is for the ledger to see to before recording anything here (see
//! [`tracking`](super::tracking) for why the order of writes keeps that so
//! across a crash).
//!
//! What's recorded must never be past where a stage's state actually is: a
//! stage recorded behind gets the difference committed again, which its
//! commits being idempotent makes harmless, but one recorded ahead is never
//! caught up.  So a stage is recorded after it moves forward and before it
//! moves back, and only the core gets to record it.

use std::collections::BTreeMap;
use std::fmt::{self, Debug};

use strata_gchain_types::*;

use crate::errors::{GExecError, GExecResult};
use crate::store::ExecutorStore;

/// Where a stage's aggregated state is, and where its history starts.
struct StagePosition<S: GChainSpec> {
    /// The last node the stage has processed and committed.
    committed_node: NodeRef<S>,

    /// The node the stage was initialized at.  It has nothing for the links
    /// committed before it.
    inited_at: NodeRef<S>,
}

impl<S: GChainSpec> Clone for StagePosition<S> {
    fn clone(&self) -> Self {
        Self {
            committed_node: self.committed_node.clone(),
            inited_at: self.inited_at.clone(),
        }
    }
}

/// Where each stage stands along the committed path.
///
/// A stage that has never been initialized isn't here at all.
///
/// This is stored apart from the [`CommitTrackingState`](super::CommitTrackingState), and what keeps the two
/// consistent is the order they're written in: a stage's node is always on
/// the committed path, because the path is extended before any stage moves
/// forward over it, and every stage is moved back before the path is cut
/// behind it.  A stage can lag the path's terminal, and only while a commit
/// is in progress; a commit cut short is finished by bringing the stages
/// level on open.
pub struct StageTrackingState<S: GChainSpec> {
    stages: BTreeMap<ProcId, StagePosition<S>>,
}

impl<S: GChainSpec> StageTrackingState<S> {
    /// Tracking with no stage initialized yet.
    pub fn new() -> Self {
        Self {
            stages: BTreeMap::new(),
        }
    }

    /// The node a processor stage has committed up to, if it has ever been
    /// initialized.
    pub fn get_stage_node(&self, proc_id: ProcId) -> Option<&NodeRef<S>> {
        self.stages.get(&proc_id).map(|pos| &pos.committed_node)
    }

    /// The node a processor stage was initialized at, if it ever has been.
    pub fn get_stage_floor(&self, proc_id: ProcId) -> Option<&NodeRef<S>> {
        self.stages.get(&proc_id).map(|pos| &pos.inited_at)
    }

    /// Records the node a stage has committed up to.  The first node recorded
    /// for a stage is the one it was initialized at.
    pub fn set_stage_node(&mut self, proc_id: ProcId, node: NodeRef<S>) {
        match self.stages.get_mut(&proc_id) {
            Some(pos) => pos.committed_node = node,
            None => {
                let pos = StagePosition {
                    committed_node: node.clone(),
                    inited_at: node,
                };
                self.stages.insert(proc_id, pos);
            }
        }
    }
}

impl<S: GChainSpec> Default for StageTrackingState<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: GChainSpec> Debug for StagePosition<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StagePosition")
            .field("node", &self.committed_node)
            .field("floor", &self.inited_at)
            .finish()
    }
}

impl<S: GChainSpec> Clone for StageTrackingState<S> {
    fn clone(&self) -> Self {
        Self {
            stages: self.stages.clone(),
        }
    }
}

impl<S: GChainSpec> Debug for StageTrackingState<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StageTracking")
            .field("stages", &self.stages)
            .finish()
    }
}

/// How far each stage has committed.
pub(crate) struct StageTracker<S: GChainSpec> {
    /// Only ever mutated through [`Self::set_stage_node`], which persists
    /// it, so it never drifts from what the store has.
    state: StageTrackingState<S>,
}

impl<S: GChainSpec> StageTracker<S> {
    /// Opens what a store holds, with no stage recorded if it never has
    /// been.
    pub(super) fn open(store: &impl ExecutorStore<Spec = S>) -> GExecResult<Self> {
        let tracking = store
            .load_stage_tracking()
            .map_err(GExecError::Storage)?
            .unwrap_or_default();
        Ok(Self { state: tracking })
    }

    /// The node a stage has committed up to, if it has ever been
    /// initialized.
    pub(crate) fn get_stage_node(&self, proc_id: ProcId) -> Option<&NodeRef<S>> {
        self.state.get_stage_node(proc_id)
    }

    /// The node a stage was initialized at, if it ever has been.
    pub(crate) fn get_stage_floor(&self, proc_id: ProcId) -> Option<&NodeRef<S>> {
        self.state.get_stage_floor(proc_id)
    }

    /// Records the node a stage stands at.  The first node recorded for a
    /// stage is the one it was initialized at.
    pub(super) fn set_stage_node(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        proc_id: ProcId,
        node: NodeRef<S>,
    ) -> GExecResult<()> {
        // Nothing is kept that isn't stored, so the change is made to a copy
        // and only taken on once the store has it.
        let mut tracking = self.state.clone();
        tracking.set_stage_node(proc_id, node);
        store
            .store_stage_tracking(&tracking)
            .map_err(GExecError::Storage)?;
        self.state = tracking;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::mem_store::MemExecutorStore;
    use crate::test_support::*;

    #[test]
    fn test_stage_floor_stays_where_stage_was_first_put() {
        let proc_id = ProcId::from_str("a").expect("test: parse ProcId");
        let mut stages = StageTrackingState::<TestSpec>::new();

        stages.set_stage_node(proc_id, TestRef(2));
        stages.set_stage_node(proc_id, TestRef(3));

        assert_eq!(stages.get_stage_node(proc_id), Some(&TestRef(3)));
        assert_eq!(stages.get_stage_floor(proc_id), Some(&TestRef(2)));
    }

    /// What's recorded is in the store before it's taken on, so a reopened
    /// tracker picks up exactly what was recorded.
    #[test]
    fn test_recorded_nodes_come_back_on_reopen() {
        let a = ProcId::from_str("a").expect("test: parse ProcId");
        let store = MemExecutorStore::<TestSpec>::new();
        let mut tracker = StageTracker::open(&store).expect("test: open");
        assert_eq!(tracker.get_stage_node(a), None);

        tracker
            .set_stage_node(&store, a, TestRef(2))
            .expect("test: set stage node");
        tracker
            .set_stage_node(&store, a, TestRef(3))
            .expect("test: set stage node");

        let reopened = StageTracker::open(&store).expect("test: reopen");
        assert_eq!(reopened.get_stage_node(a), Some(&TestRef(3)));
        assert_eq!(reopened.get_stage_floor(a), Some(&TestRef(2)));
    }
}
