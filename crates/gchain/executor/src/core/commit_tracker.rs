//! The committed path and the log of commits that built it, kept in step
//! with the store.
//!
//! This knows nothing of artifacts, of the stages, or of where they stand:
//! it's told which paths are committed and where the path is cut, and keeps
//! the record of that consistent with the store it's handed, which has to be
//! the one it was opened from.
//!
//! Every change is written such that being cut short leaves at worst commit
//! segments the tracking state doesn't cover, which are trimmed off on open.

use std::fmt::{self, Debug};
use strata_gchain_types::*;

use super::history::{CommitWalker, LogCut, find_log_cut, find_segments_between};
use super::tracking::CommitIndex;
use crate::errors::{GExecError, GExecResult};
use crate::store::ExecutorStore;

/// The committed path and the bounds of the commit log it was built from.
pub struct CommitTrackingState<S: GChainSpec> {
    /// The committed path, from the oldest node still rolled back to up to
    /// the committed node.
    committed: LinkPath<S>,

    /// The oldest commit segment still covering part of the committed path.
    first_commit: CommitIndex,

    /// The index the next commit goes under.
    next_commit: CommitIndex,
}

impl<S: GChainSpec> CommitTrackingState<S> {
    /// Tracking for a pipeline sitting at a node with nothing committed past
    /// it.
    pub fn new_at(base_node: NodeRef<S>) -> Self {
        Self {
            committed: LinkPath::new_at(base_node),
            first_commit: CommitIndex::first(),
            next_commit: CommitIndex::first(),
        }
    }

    pub fn committed_node(&self) -> &NodeRef<S> {
        self.committed.terminal_node()
    }

    pub fn committed_path(&self) -> &LinkPath<S> {
        &self.committed
    }

    /// The oldest commit segment still covering part of the committed path,
    /// if there is one.
    pub fn first_commit(&self) -> CommitIndex {
        self.first_commit
    }

    /// The index the next commit goes under.
    pub fn next_commit(&self) -> CommitIndex {
        self.next_commit
    }

    /// The newest commit segment, if anything is committed.
    pub fn last_commit(&self) -> Option<CommitIndex> {
        self.next_commit
            .prev()
            .filter(|last| *last >= self.first_commit)
    }

    pub fn is_committed(&self, lref: &LinkRef<S>) -> bool {
        self.committed.links().contains(lref)
    }

    /// The committed links from a node on the committed path up to the
    /// committed node.
    pub fn committed_path_from(&self, node: &NodeRef<S>) -> GExecResult<LinkPath<S>> {
        let idx = self.get_committed_index_of(node)?;
        Ok(self.committed.slice(idx, self.committed.len()))
    }

    /// Extends the committed path by a path continuing from the committed
    /// node, as the commit stored under [`Self::next_commit`].
    pub fn extend_committed(&mut self, path: &LinkPath<S>) {
        let extended = self.committed.try_extend(path);
        debug_assert!(
            extended,
            "gchain: committed path continues from its terminal"
        );
        self.next_commit = self.next_commit.next();
    }

    /// Cuts the committed path back to a node on it, with the index the next
    /// commit goes under once the commit log is cut back to match.
    pub fn truncate_committed_to(
        &mut self,
        node: &NodeRef<S>,
        next_commit: CommitIndex,
    ) -> GExecResult<()> {
        let idx = self.get_committed_index_of(node)?;
        self.committed = self.committed.slice(0, idx);
        self.next_commit = next_commit;
        Ok(())
    }

    /// Moves the oldest node the pipeline can roll back to forward to a node
    /// on the committed path, giving up the links before it, with the oldest
    /// commit segment left once the commit log is cut to match.
    pub fn advance_base_to(
        &mut self,
        node: &NodeRef<S>,
        first_commit: CommitIndex,
    ) -> GExecResult<()> {
        let idx = self.get_committed_index_of(node)?;
        self.committed = self.committed.slice(idx, self.committed.len());
        self.first_commit = first_commit;
        Ok(())
    }

    fn get_committed_index_of(&self, node: &NodeRef<S>) -> GExecResult<usize> {
        self.committed
            .get_node_index(node)
            .ok_or_else(|| GExecError::node_not_on_committed_path(node))
    }
}

// Bounds fall on the ref types, not the marker spec type.
impl<S: GChainSpec> Clone for CommitTrackingState<S> {
    fn clone(&self) -> Self {
        Self {
            committed: self.committed.clone(),
            first_commit: self.first_commit,
            next_commit: self.next_commit,
        }
    }
}

impl<S: GChainSpec> Debug for CommitTrackingState<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommitTracking")
            .field("committed", &self.committed)
            .field("first_commit", &self.first_commit)
            .field("next_commit", &self.next_commit)
            .finish()
    }
}

/// The committed path and the commit segments it was built from.
pub(crate) struct CommitTracker<S: GChainSpec> {
    /// Only ever mutated through [`Self::update_tracking`], which persists
    /// it, so it never drifts from what the store has.
    state: CommitTrackingState<S>,
}

impl<S: GChainSpec> CommitTracker<S> {
    /// Opens the log a store holds, starting it off at `base_node` if the
    /// store has never been used, and trims off whatever a change that was
    /// cut short left behind.
    pub(super) fn open(
        store: &impl ExecutorStore<Spec = S>,
        base_node: NodeRef<S>,
    ) -> GExecResult<Self> {
        let tracking = load_or_init_tracking(store, base_node)?;
        trim_commit_log(store, &tracking)?;
        Ok(Self { state: tracking })
    }

    pub(crate) fn committed_node(&self) -> &NodeRef<S> {
        self.state.committed_node()
    }

    pub(crate) fn committed_path(&self) -> &LinkPath<S> {
        self.state.committed_path()
    }

    /// A walker over the commits, starting from the newest.
    pub(crate) fn start_walker<'s, X: ExecutorStore<Spec = S>>(
        &self,
        store: &'s X,
    ) -> CommitWalker<'s, X> {
        CommitWalker::from_tracking(store, &self.state)
    }

    /// Where a node sits on the committed path, if it's on it.
    pub(crate) fn get_committed_index(&self, node: &NodeRef<S>) -> Option<usize> {
        self.committed_path().get_node_index(node)
    }

    pub(crate) fn is_committed(&self, lref: &LinkRef<S>) -> bool {
        self.state.is_committed(lref)
    }

    /// See [`CommitTrackingState::committed_path_from`].
    pub(crate) fn committed_path_from(&self, node: &NodeRef<S>) -> GExecResult<LinkPath<S>> {
        self.state.committed_path_from(node)
    }

    /// The commits covering the committed links between two nodes on the
    /// committed path, oldest first, with the ones at either end cut down to
    /// the part in between.
    pub(crate) fn find_segments_between(
        &self,
        store: &impl ExecutorStore<Spec = S>,
        from: &NodeRef<S>,
        to: &NodeRef<S>,
    ) -> GExecResult<Vec<LinkPath<S>>> {
        find_segments_between(self.start_walker(store), from, to)
    }

    /// Commits a path continuing from the committed node: it's written as
    /// the next segment of the log and the committed path is extended over
    /// it, before any stage has committed it.
    ///
    /// The stages then lag the committed node until they're brought level,
    /// which a commit being cut short leaves to the next open.  If the
    /// segment goes in but the path isn't extended, the segment is beyond
    /// what the tracking state covers and is dropped then instead.
    pub(super) fn append(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        path: &LinkPath<S>,
    ) -> GExecResult<()> {
        if path.base_node() != self.committed_node() {
            return Err(GExecError::node_not_on_committed_path(path.base_node()));
        }

        store
            .store_commit_segment(self.state.next_commit(), path)
            .map_err(GExecError::Storage)?;
        self.update_tracking(store, |tracking| {
            tracking.extend_committed(path);
            Ok(())
        })
    }

    /// Cuts the committed path and the log back to a node on the path.  The
    /// stages have to have been rolled back to it already.
    pub(super) fn truncate_to(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        node: &NodeRef<S>,
    ) -> GExecResult<()> {
        self.check_on_committed_path(node)?;
        let cut = self.find_log_cut(store, node)?;
        let next_commit = match &cut {
            None => self.state.next_commit(),
            Some(cut) if cut.is_at_start() => cut.idx(),
            Some(cut) => cut.idx().next(),
        };

        self.update_tracking(store, |tracking| {
            tracking.truncate_committed_to(node, next_commit)
        })?;

        if let Some(cut) = cut
            && !cut.is_at_start()
            && !cut.is_at_end()
        {
            store
                .store_commit_segment(cut.idx(), &cut.get_part_before())
                .map_err(GExecError::Storage)?;
        }
        store
            .discard_commit_segments_from(next_commit)
            .map_err(GExecError::Storage)
    }

    /// Moves the base of the committed path and the start of the log forward
    /// to a node on the path, giving up the links before it.
    pub(super) fn advance_base_to(
        &mut self,
        store: &impl ExecutorStore<Spec = S>,
        node: &NodeRef<S>,
    ) -> GExecResult<()> {
        self.check_on_committed_path(node)?;
        let cut = self.find_log_cut(store, node)?;
        let first_commit = match &cut {
            None => self.state.first_commit(),
            Some(cut) if cut.is_at_end() => cut.idx().next(),
            Some(cut) => cut.idx(),
        };

        self.update_tracking(store, |tracking| {
            tracking.advance_base_to(node, first_commit)
        })?;

        if let Some(cut) = cut
            && !cut.is_at_start()
            && !cut.is_at_end()
        {
            store
                .store_commit_segment(cut.idx(), &cut.get_part_after())
                .map_err(GExecError::Storage)?;
        }
        store
            .discard_commit_segments_before(first_commit)
            .map_err(GExecError::Storage)
    }

    /// Finds the newest commit a node on the committed path is part of, if
    /// anything is committed.
    fn find_log_cut(
        &self,
        store: &impl ExecutorStore<Spec = S>,
        node: &NodeRef<S>,
    ) -> GExecResult<Option<LogCut<S>>> {
        match find_log_cut(self.start_walker(store), node)? {
            Some(cut) => Ok(Some(cut)),
            None if self.committed_path().is_empty() => Ok(None),
            None => Err(GExecError::corrupt_commit_log(node)),
        }
    }

    fn check_on_committed_path(&self, node: &NodeRef<S>) -> GExecResult<()> {
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
        store: &impl ExecutorStore<Spec = S>,
        update: impl FnOnce(&mut CommitTrackingState<S>) -> GExecResult<()>,
    ) -> GExecResult<()> {
        // For safety, clone the tracking state and apply the changes to that,
        // only overwrite the field if the store is successful.
        let mut tracking = self.state.clone();
        update(&mut tracking)?;
        store
            .store_commit_tracking(&tracking)
            .map_err(GExecError::Storage)?;
        self.state = tracking;
        Ok(())
    }
}

/// Loads where the pipeline stands, starting it off at `base_node` if the
/// store has never been used.
fn load_or_init_tracking<S: GChainSpec>(
    store: &impl ExecutorStore<Spec = S>,
    base_node: NodeRef<S>,
) -> GExecResult<CommitTrackingState<S>> {
    if let Some(tracking) = store.load_commit_tracking().map_err(GExecError::Storage)? {
        return Ok(tracking);
    }

    let tracking = CommitTrackingState::new_at(base_node);
    store
        .store_commit_tracking(&tracking)
        .map_err(GExecError::Storage)?;
    Ok(tracking)
}

/// Drops whatever the commit log has beyond what the tracking state covers,
/// which is what a commit, rollback, or prune that was cut short leaves
/// behind.
fn trim_commit_log<S: GChainSpec>(
    store: &impl ExecutorStore<Spec = S>,
    tracking: &CommitTrackingState<S>,
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

fn load_commit_segment<S: GChainSpec>(
    store: &impl ExecutorStore<Spec = S>,
    idx: CommitIndex,
) -> GExecResult<LinkPath<S>> {
    store
        .load_commit_segment(idx)
        .map_err(GExecError::Storage)?
        .ok_or(GExecError::MissingCommitSegment(idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem_store::MemExecutorStore;
    use crate::test_support::*;

    /// A path from a base node through links given as `(lref, target)`.
    fn path(base: u8, steps: &[(u8, u8)]) -> LinkPath<TestSpec> {
        LinkPath::from_steps(
            TestRef(base),
            steps.iter().map(|(l, n)| (TestRef(*l), TestRef(*n))),
        )
    }

    fn open(store: &MemExecutorStore<TestSpec>) -> CommitTracker<TestSpec> {
        CommitTracker::open(store, TestRef(1)).expect("test: open log")
    }

    fn segments(store: &MemExecutorStore<TestSpec>) -> Vec<Option<Vec<TestRef>>> {
        (0..3).map(|idx| stored_segment(store, idx)).collect()
    }

    #[test]
    fn test_committed_path_edits() {
        let mut tracking = CommitTrackingState::<TestSpec>::new_at(TestRef(1));

        tracking.extend_committed(&path(1, &[(10, 2), (11, 3)]));
        assert_eq!(tracking.committed_node(), &TestRef(3));
        assert!(tracking.is_committed(&TestRef(11)));
        assert!(!tracking.is_committed(&TestRef(20)));

        let suffix = tracking
            .committed_path_from(&TestRef(2))
            .expect("test: suffix");
        assert_eq!(suffix, path(2, &[(11, 3)]));
        let err = expect_err(
            tracking.committed_path_from(&TestRef(9)),
            "node off the path to be refused",
        );
        assert!(matches!(err, GExecError::NodeNotOnCommittedPath(_)));

        assert_eq!(tracking.last_commit(), Some(CommitIndex::from(0)));

        tracking
            .truncate_committed_to(&TestRef(2), CommitIndex::from(1))
            .expect("test: truncate");
        assert_eq!(*tracking.committed_path(), path(1, &[(10, 2)]));

        tracking.extend_committed(&suffix);
        assert_eq!(tracking.last_commit(), Some(CommitIndex::from(1)));
        tracking
            .advance_base_to(&TestRef(2), CommitIndex::from(1))
            .expect("test: advance base");
        assert_eq!(*tracking.committed_path(), path(2, &[(11, 3)]));
        assert_eq!(tracking.first_commit(), CommitIndex::from(1));
        let err = expect_err(
            tracking.truncate_committed_to(&TestRef(1), CommitIndex::from(1)),
            "node before the base to be refused",
        );
        assert!(matches!(err, GExecError::NodeNotOnCommittedPath(_)));
    }

    /// A segment that was written but never covered by the tracking state
    /// goes when the log is next opened.
    #[test]
    fn test_uncovered_segment_is_dropped_on_reopen() {
        let store = MemExecutorStore::new();
        let mut log = open(&store);
        log.append(&store, &path(1, &[(10, 2)]))
            .expect("test: append");
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
        let store = MemExecutorStore::new();
        let mut log = open(&store);
        log.append(&store, &path(1, &[(10, 2), (11, 3)]))
            .expect("test: append");
        log.append(&store, &path(3, &[(12, 4), (13, 5)]))
            .expect("test: append");

        log.truncate_to(&store, &TestRef(4))
            .expect("test: truncate");
        log.advance_base_to(&store, &TestRef(2))
            .expect("test: advance");

        assert_eq!(log.committed_path(), &path(2, &[(11, 3), (12, 4)]));
        assert_eq!(
            segments(&store),
            vec![Some(vec![TestRef(11)]), Some(vec![TestRef(12)]), None]
        );
        let between = log
            .find_segments_between(&store, &TestRef(2), &TestRef(4))
            .expect("test: find segments");
        assert_eq!(between, vec![path(2, &[(11, 3)]), path(3, &[(12, 4)])]);

        // What the store has is what a reopened log picks up from.
        let reopened = open(&store);
        assert_eq!(reopened.committed_path(), log.committed_path());
    }
}
