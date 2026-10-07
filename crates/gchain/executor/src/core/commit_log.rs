//! Where the pipeline stands and the log of commits that got it there, kept
//! in step with the store.
//!
//! This is the positional half of an executor's bookkeeping.  It reads and
//! writes only the store, and knows nothing of artifacts or of what the
//! stages do: it's told where each stage stands and which paths are
//! committed, and keeps the record of that consistent.  Only the core gets
//! to tell it, so that what's recorded about a stage is never more than the
//! stage has done.
//!
//! Every change is written such that being cut short leaves at worst commit
//! segments the tracking state doesn't cover, which are trimmed off on open.

use std::sync::Arc;

use strata_gchain_types::*;

use super::history::{CommitWalker, LogCut, find_log_cut, find_segments_between};
use crate::errors::{GExecError, GExecResult};
use crate::store::ExecutorStore;
use crate::tracking::{CommitIndex, TrackingState};

/// The committed path, how far each stage has committed along it, and the
/// commit segments it was built from.
pub(crate) struct CommitLog<X: ExecutorStore> {
    /// Where the pipeline stands.
    ///
    /// Only ever mutated through [`Self::update_tracking`], which persists
    /// it, so it never drifts from what the store has.
    tracking: TrackingState<X::Spec>,

    store: Arc<X>,
}

impl<X: ExecutorStore> CommitLog<X> {
    /// Opens the log a store holds, starting it off at `base_node` if the
    /// store has never been used, and trims off whatever a change that was
    /// cut short left behind.
    pub(super) fn open(store: Arc<X>, base_node: NodeRef<X::Spec>) -> GExecResult<Self> {
        let tracking = load_or_init_tracking(store.as_ref(), base_node)?;
        trim_commit_log(store.as_ref(), &tracking)?;
        Ok(Self { tracking, store })
    }

    pub(crate) fn tracking(&self) -> &TrackingState<X::Spec> {
        &self.tracking
    }

    pub(crate) fn committed_node(&self) -> &NodeRef<X::Spec> {
        self.tracking.committed_node()
    }

    pub(crate) fn committed_path(&self) -> &LinkPath<X::Spec> {
        self.tracking.committed_path()
    }

    /// A walker over the commits, starting from the newest.
    pub(crate) fn walker(&self) -> CommitWalker<'_, X> {
        CommitWalker::from_tracking(self.store.as_ref(), &self.tracking)
    }

    /// Where a node sits on the committed path, if it's on it.
    pub(crate) fn get_committed_index(&self, node: &NodeRef<X::Spec>) -> Option<usize> {
        self.committed_path().get_node_index(node)
    }

    /// Where a stage stands on the committed path, if it has been
    /// initialized.
    ///
    /// A stage that's been initialized always stands somewhere on the path:
    /// the path is only ever cut back to where every stage is at or before,
    /// and its base only ever moved up to where every stage is at or past.
    pub(crate) fn get_stage_index(&self, proc_id: ProcId) -> Option<usize> {
        let node = self.tracking.get_stage_node(proc_id)?;
        let idx = self
            .get_committed_index(node)
            .expect("gchain: stage node on committed path");
        Some(idx)
    }

    /// The commits covering the committed links between two nodes on the
    /// committed path, oldest first, with the ones at either end cut down to
    /// the part in between.
    pub(crate) fn find_segments_between(
        &self,
        from: &NodeRef<X::Spec>,
        to: &NodeRef<X::Spec>,
    ) -> GExecResult<Vec<LinkPath<X::Spec>>> {
        find_segments_between(self.walker(), from, to)
    }

    /// Records the node a stage stands at, which has to be on the committed
    /// path.
    ///
    /// What's recorded must never be past where the stage's state actually
    /// is: a stage recorded behind gets the difference committed again,
    /// which its commits being idempotent makes harmless, but one recorded
    /// ahead is never caught up.  So a stage is recorded after it moves
    /// forward and before it moves back.
    pub(super) fn set_stage_node(
        &mut self,
        proc_id: ProcId,
        node: NodeRef<X::Spec>,
    ) -> GExecResult<()> {
        self.check_on_committed_path(&node)?;
        self.update_tracking(|tracking| {
            tracking.set_stage_node(proc_id, node);
            Ok(())
        })
    }

    /// Commits a path continuing from the committed node: it's written as
    /// the next segment of the log and the committed path is extended over
    /// it, before any stage has committed it.
    ///
    /// The stages then lag the committed node until they're brought level,
    /// which a commit being cut short leaves to the next open.  If the
    /// segment goes in but the path isn't extended, the segment is beyond
    /// what the tracking state covers and is dropped then instead.
    pub(super) fn append(&mut self, path: &LinkPath<X::Spec>) -> GExecResult<()> {
        if path.base_node() != self.committed_node() {
            return Err(GExecError::node_not_on_committed_path(path.base_node()));
        }

        self.store
            .store_commit_segment(self.tracking.next_commit(), path)
            .map_err(GExecError::Storage)?;
        self.update_tracking(|tracking| {
            tracking.extend_committed(path);
            Ok(())
        })
    }

    /// Cuts the committed path and the log back to a node on the path.  The
    /// stages have to have been rolled back to it already.
    pub(super) fn truncate_to(&mut self, node: &NodeRef<X::Spec>) -> GExecResult<()> {
        self.check_on_committed_path(node)?;
        let cut = self.find_log_cut(node)?;
        let next_commit = match &cut {
            None => self.tracking.next_commit(),
            Some(cut) if cut.is_at_start() => cut.idx(),
            Some(cut) => cut.idx().next(),
        };

        self.update_tracking(|tracking| tracking.truncate_committed_to(node, next_commit))?;

        if let Some(cut) = cut
            && !cut.is_at_start()
            && !cut.is_at_end()
        {
            self.store
                .store_commit_segment(cut.idx(), &cut.get_part_before())
                .map_err(GExecError::Storage)?;
        }
        self.store
            .discard_commit_segments_from(next_commit)
            .map_err(GExecError::Storage)
    }

    /// Moves the base of the committed path and the start of the log forward
    /// to a node on the path, giving up the links before it.
    pub(super) fn advance_base_to(&mut self, node: &NodeRef<X::Spec>) -> GExecResult<()> {
        self.check_on_committed_path(node)?;
        let cut = self.find_log_cut(node)?;
        let first_commit = match &cut {
            None => self.tracking.first_commit(),
            Some(cut) if cut.is_at_end() => cut.idx().next(),
            Some(cut) => cut.idx(),
        };

        self.update_tracking(|tracking| tracking.advance_base_to(node, first_commit))?;

        if let Some(cut) = cut
            && !cut.is_at_start()
            && !cut.is_at_end()
        {
            self.store
                .store_commit_segment(cut.idx(), &cut.get_part_after())
                .map_err(GExecError::Storage)?;
        }
        self.store
            .discard_commit_segments_before(first_commit)
            .map_err(GExecError::Storage)
    }

    /// Finds the newest commit a node on the committed path is part of, if
    /// anything is committed.
    fn find_log_cut(&self, node: &NodeRef<X::Spec>) -> GExecResult<Option<LogCut<X::Spec>>> {
        match find_log_cut(self.walker(), node)? {
            Some(cut) => Ok(Some(cut)),
            None if self.committed_path().is_empty() => Ok(None),
            None => Err(GExecError::corrupt_commit_log(node)),
        }
    }

    fn check_on_committed_path(&self, node: &NodeRef<X::Spec>) -> GExecResult<()> {
        self.committed_path()
            .get_node_index(node)
            .is_some()
            .ok_or_else(|| GExecError::node_not_on_committed_path(node))
    }

    /// Applies a change to the tracking state and persists the result.
    ///
    /// This is the only way `self.tracking` gets mutated, so that no change
    /// can be made and then forgotten to be stored.  An update that fails
    /// must leave the state as it found it, since nothing is stored then.
    fn update_tracking(
        &mut self,
        update: impl FnOnce(&mut TrackingState<X::Spec>) -> GExecResult<()>,
    ) -> GExecResult<()> {
        // For safety, clone the tracking state and apply the changes to that,
        // only overwrite the field if the store is successful.
        let mut tracking = self.tracking.clone();
        update(&mut tracking)?;
        self.store
            .store_tracking(&tracking)
            .map_err(GExecError::Storage)?;
        self.tracking = tracking;
        Ok(())
    }
}

/// Loads where the pipeline stands, starting it off at `base_node` if the
/// store has never been used.
fn load_or_init_tracking<X: ExecutorStore>(
    store: &X,
    base_node: NodeRef<X::Spec>,
) -> GExecResult<TrackingState<X::Spec>> {
    if let Some(tracking) = store.load_tracking().map_err(GExecError::Storage)? {
        return Ok(tracking);
    }

    let tracking = TrackingState::new_at(base_node);
    store
        .store_tracking(&tracking)
        .map_err(GExecError::Storage)?;
    Ok(tracking)
}

/// Drops whatever the commit log has beyond what the tracking state covers,
/// which is what a commit, rollback, or prune that was cut short leaves
/// behind.
fn trim_commit_log<X: ExecutorStore>(
    store: &X,
    tracking: &TrackingState<X::Spec>,
) -> GExecResult<()> {
    store
        .discard_commit_segments_from(tracking.next_commit())
        .map_err(GExecError::Storage)?;
    store
        .discard_commit_segments_before(tracking.first_commit())
        .map_err(GExecError::Storage)?;

    let Some(last) = tracking.last_commit() else {
        return Ok(());
    };

    let committed = tracking.committed_node();
    let newest = load_commit_segment(store, last)?;
    if newest.terminal_node() != committed {
        let pos = newest
            .get_node_index(committed)
            .ok_or_else(|| GExecError::corrupt_commit_log(committed))?;
        store
            .store_commit_segment(last, &newest.slice(0, pos))
            .map_err(GExecError::Storage)?;
    }

    let first = tracking.first_commit();
    let base = tracking.committed_path().base_node();
    let oldest = load_commit_segment(store, first)?;
    if oldest.base_node() != base {
        let pos = oldest
            .get_node_index(base)
            .ok_or_else(|| GExecError::corrupt_commit_log(base))?;
        store
            .store_commit_segment(first, &oldest.slice(pos, oldest.len()))
            .map_err(GExecError::Storage)?;
    }
    Ok(())
}

fn load_commit_segment<X: ExecutorStore>(
    store: &X,
    idx: CommitIndex,
) -> GExecResult<LinkPath<X::Spec>> {
    store
        .load_commit_segment(idx)
        .map_err(GExecError::Storage)?
        .ok_or(GExecError::MissingCommitSegment(idx))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::mem_store::MemExecutorStore;
    use crate::test_support::*;

    type Log = CommitLog<MemExecutorStore<TestSpec>>;

    /// A path from a base node through links given as `(lref, target)`.
    fn path(base: u8, steps: &[(u8, u8)]) -> LinkPath<TestSpec> {
        LinkPath::from_steps(
            TestRef(base),
            steps.iter().map(|(l, n)| (TestRef(*l), TestRef(*n))),
        )
    }

    fn open(store: &Arc<MemExecutorStore<TestSpec>>) -> Log {
        CommitLog::open(store.clone(), TestRef(1)).expect("test: open log")
    }

    fn segments(store: &MemExecutorStore<TestSpec>) -> Vec<Option<Vec<TestRef>>> {
        (0..3).map(|idx| stored_segment(store, idx)).collect()
    }

    /// A segment that was written but never covered by the tracking state
    /// goes when the log is next opened.
    #[test]
    fn test_uncovered_segment_is_dropped_on_reopen() {
        let store = Arc::new(MemExecutorStore::new());
        let mut log = open(&store);
        log.append(&path(1, &[(10, 2)])).expect("test: append");
        store
            .store_commit_segment(CommitIndex::from(1), &path(2, &[(11, 3), (12, 4)]))
            .expect("test: store segment");

        let log = open(&store);

        assert_eq!(log.committed_node(), &TestRef(2));
        assert_eq!(segments(&store), vec![Some(vec![TestRef(10)]), None, None]);
    }

    /// The log is cut in step with the committed path at either end, through
    /// the middle of a commit if that's where the node falls.
    #[test]
    fn test_truncating_and_advancing_cut_the_segments_to_match() {
        let store = Arc::new(MemExecutorStore::new());
        let mut log = open(&store);
        log.append(&path(1, &[(10, 2), (11, 3)]))
            .expect("test: append");
        log.append(&path(3, &[(12, 4), (13, 5)]))
            .expect("test: append");
        let a = ProcId::from_str("a").expect("test: parse ProcId");
        log.set_stage_node(a, TestRef(4))
            .expect("test: set stage node");

        log.truncate_to(&TestRef(4)).expect("test: truncate");
        log.advance_base_to(&TestRef(2)).expect("test: advance");

        assert_eq!(log.committed_path(), &path(2, &[(11, 3), (12, 4)]));
        assert_eq!(
            segments(&store),
            vec![Some(vec![TestRef(11)]), Some(vec![TestRef(12)]), None]
        );
        let between = log
            .find_segments_between(&TestRef(2), &TestRef(4))
            .expect("test: find segments");
        assert_eq!(between, vec![path(2, &[(11, 3)]), path(3, &[(12, 4)])]);
        assert_eq!(log.get_stage_index(a), Some(2));

        // What the store has is what a reopened log picks up from.
        let reopened = open(&store);
        assert_eq!(reopened.committed_path(), log.committed_path());
        assert_eq!(reopened.tracking().get_stage_node(a), Some(&TestRef(4)));
    }
}
