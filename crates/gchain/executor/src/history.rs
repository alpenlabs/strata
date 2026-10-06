//! Walking back over what was committed.
//!
//! The store keeps every commit as the path it covered.  This module reads
//! them newest first, a segment at a time, and works out from a verdict on
//! each committed link where something that has to redo part of the history
//! should pick up from.  It knows nothing about stages or artifacts: what
//! makes a link need redoing is up to whoever is asking.

use strata_gchain_types::*;

use crate::errors::GExecError;
use crate::store::ExecutorStore;
use crate::tracking::{CommitIndex, TrackingState};

/// A commit out of the log with the path it covered.
pub(crate) struct CommitSegment<S: GChainSpec> {
    idx: CommitIndex,
    path: LinkPath<S>,
}

impl<S: GChainSpec> CommitSegment<S> {
    pub(crate) fn idx(&self) -> CommitIndex {
        self.idx
    }

    pub(crate) fn path(&self) -> &LinkPath<S> {
        &self.path
    }

    pub(crate) fn into_path(self) -> LinkPath<S> {
        self.path
    }
}

/// A committed link with where it sits in the graph.
// TODO(trey): convert this into LinkInfo structure in the types crate and reuse in various places
pub(crate) struct CommittedStep<S: GChainSpec> {
    lref: LinkRef<S>,
    endpoints: LinkEndpoints<S>,
}

impl<S: GChainSpec> CommittedStep<S> {
    pub(crate) fn lref(&self) -> &LinkRef<S> {
        &self.lref
    }

    pub(crate) fn origin(&self) -> &NodeRef<S> {
        self.endpoints.origin()
    }

    pub(crate) fn target(&self) -> &NodeRef<S> {
        self.endpoints.target()
    }
}

/// Reads the commit log newest first, loading one segment at a time.
pub(crate) struct CommitWalker<'s, X: ExecutorStore> {
    store: &'s X,
    first: CommitIndex,

    /// The next segment to load, if there are any left.
    next_idx: Option<CommitIndex>,

    /// What's left of the segment being stepped through, oldest first.
    steps: Vec<CommittedStep<X::Spec>>,
}

impl<'s, X: ExecutorStore> CommitWalker<'s, X> {
    /// A walker over the commits between two indexes, starting from the
    /// newest.  There's nothing to walk without a newest one.
    pub(crate) fn new(store: &'s X, first: CommitIndex, last: Option<CommitIndex>) -> Self {
        Self {
            store,
            first,
            next_idx: last.filter(|last| *last >= first),
            steps: Vec::new(),
        }
    }

    /// Constructs a new instance from the committed indexes of a [`TrackingState`].
    pub(crate) fn from_tracking(store: &'s X, tracking: &TrackingState<X::Spec>) -> Self {
        Self::new(store, tracking.first_commit(), tracking.last_commit())
    }

    /// The next commit back with the path it covered.
    ///
    /// Drops whatever was left of the segment being stepped through.
    pub(crate) fn next_segment(&mut self) -> Result<Option<CommitSegment<X::Spec>>, GExecError> {
        self.steps.clear();
        let Some(idx) = self.next_idx else {
            return Ok(None);
        };

        let path = self
            .store
            .load_commit_segment(idx)
            .map_err(GExecError::Storage)?
            .ok_or(GExecError::MissingCommitSegment(idx))?;
        self.next_idx = idx.prev().filter(|prev| *prev >= self.first);
        Ok(Some(CommitSegment { idx, path }))
    }

    /// The next committed link back.
    pub(crate) fn next_step(&mut self) -> Result<Option<CommittedStep<X::Spec>>, GExecError> {
        while self.steps.is_empty() {
            let Some(segment) = self.next_segment()? else {
                return Ok(None);
            };
            self.steps = segment_steps(&segment);
        }
        Ok(self.steps.pop())
    }
}

// TODO(trey): make this into a fn on LinkPath that returns an
// `impl Iterator` that we can just .collect in `next_step`
fn segment_steps<S: GChainSpec>(segment: &CommitSegment<S>) -> Vec<CommittedStep<S>> {
    let path = segment.path();
    path.nodes()
        .iter()
        .zip(path.steps())
        .map(|(origin, (lref, target))| CommittedStep {
            lref: lref.clone(),
            endpoints: LinkEndpoints::new(origin.clone(), target.clone()),
        })
        .collect()
}

/// Where a node sits in the commit log.
pub(crate) struct LogCut<S: GChainSpec> {
    idx: CommitIndex,
    segment: LinkPath<S>,
    pos: usize,
}

impl<S: GChainSpec> LogCut<S> {
    /// The commit the node is part of.
    pub(crate) fn idx(&self) -> CommitIndex {
        self.idx
    }

    /// Whether the node is the one the commit started from.
    pub(crate) fn is_at_start(&self) -> bool {
        self.pos == 0
    }

    /// Whether the node is the one the commit ended at.
    pub(crate) fn is_at_end(&self) -> bool {
        self.pos == self.segment.len()
    }

    /// The part of the commit up to the node.
    pub(crate) fn get_part_before(&self) -> LinkPath<S> {
        self.segment.slice(0, self.pos)
    }

    /// The part of the commit from the node on.
    pub(crate) fn get_part_after(&self) -> LinkPath<S> {
        self.segment.slice(self.pos, self.segment.len())
    }
}

/// Finds the newest commit a node is part of, if the log has it.
pub(crate) fn find_log_cut<X: ExecutorStore>(
    mut walker: CommitWalker<'_, X>,
    node: &NodeRef<X::Spec>,
) -> Result<Option<LogCut<X::Spec>>, GExecError> {
    while let Some(commit) = walker.next_segment()? {
        if let Some(pos) = commit.path().get_node_index(node) {
            return Ok(Some(LogCut {
                idx: commit.idx(),
                segment: commit.into_path(),
                pos,
            }));
        }
    }

    Ok(None)
}

/// The commits covering the links between two nodes in the log, oldest
/// first, with the ones at either end cut down to the part in between.
pub(crate) fn find_segments_between<X: ExecutorStore>(
    mut walker: CommitWalker<'_, X>,
    from: &NodeRef<X::Spec>,
    to: &NodeRef<X::Spec>,
) -> Result<Vec<LinkPath<X::Spec>>, GExecError> {
    if from == to {
        return Ok(Vec::new());
    }

    let mut segments = Vec::new();
    let mut reached_to = false;
    while let Some(commit) = walker.next_segment()? {
        let mut segment = commit.into_path();
        if !reached_to {
            let Some(pos) = segment.get_node_index(to) else {
                continue;
            };
            segment = segment.slice(0, pos);
            reached_to = true;
        }

        let from_pos = segment.get_node_index(from);
        if let Some(pos) = from_pos {
            segment = segment.slice(pos, segment.len());
        }
        if !segment.is_empty() {
            segments.push(segment);
        }
        if from_pos.is_some() {
            segments.reverse();
            return Ok(segments);
        }
    }

    Err(GExecError::corrupt_commit_log(from))
}

/// Verdicts on committed links and the nodes between them, from whoever wants
/// part of the history redone.
pub(crate) trait StepJudge<S: GChainSpec> {
    /// Whether what's there for a committed link can stay as it is.
    fn check_step_acceptable(&mut self, step: &CommittedStep<S>) -> Result<bool, GExecError>;

    /// Whether redoing links can start from a node.
    fn check_can_resume_at(&mut self, node: &NodeRef<S>) -> Result<bool, GExecError>;
}

/// Where redoing committed links has to start from.
pub(crate) enum ResumeSearch<S: GChainSpec> {
    /// Nothing has to be redone.
    UpToDate,

    /// The links to redo, as the path from the node to resume at up to the
    /// newest committed node.
    ResumeAt(LinkPath<S>),

    /// Links have to be redone, but nowhere they could be redone from can be
    /// resumed at.
    NoResumePoint,
}

/// Walks the commit history back from its newest node to find the links that
/// have to be redone and the node to redo them from.
///
/// The links to redo run from the newest back to the first acceptable one,
/// and further back while the node reached can't be resumed at.  This relies
/// on acceptable links never coming after unacceptable ones, so the walk
/// never has to look past the first acceptable link with nothing collected.
/// The walk doesn't go back past `floor`.
pub(crate) fn find_resume_path<S: GChainSpec, X: ExecutorStore<Spec = S>>(
    mut walker: CommitWalker<'_, X>,
    newest: &NodeRef<S>,
    floor: &NodeRef<S>,
    judge: &mut impl StepJudge<S>,
) -> Result<ResumeSearch<S>, GExecError> {
    // Newest first, with the node we've walked back to so far.
    let mut redo: Vec<(LinkRef<S>, NodeRef<S>)> = Vec::new();
    let mut node = newest.clone();

    while node != *floor {
        let Some(step) = walker.next_step()? else {
            break;
        };

        if judge.check_step_acceptable(&step)? {
            if redo.is_empty() {
                return Ok(ResumeSearch::UpToDate);
            }
            if judge.check_can_resume_at(&node)? {
                break;
            }
        }

        node = step.origin().clone();
        redo.push((step.lref().clone(), step.target().clone()));
    }

    if redo.is_empty() {
        return Ok(ResumeSearch::UpToDate);
    }
    if !judge.check_can_resume_at(&node)? {
        return Ok(ResumeSearch::NoResumePoint);
    }

    redo.reverse();
    Ok(ResumeSearch::ResumeAt(LinkPath::from_steps(node, redo)))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

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

    /// A commit log of 1→2→3 then 3→4 then 4→5→6, under indexes 3 to 5.
    fn store() -> MemExecutorStore<TestSpec> {
        let store = MemExecutorStore::new();
        let segments = [
            path(1, &[(10, 2), (11, 3)]),
            path(3, &[(12, 4)]),
            path(4, &[(13, 5), (14, 6)]),
        ];
        for (idx, segment) in (3..).zip(segments) {
            store
                .store_commit_segment(CommitIndex::from(idx), &segment)
                .expect("test: store segment");
        }
        store
    }

    fn walker(store: &MemExecutorStore<TestSpec>) -> CommitWalker<'_, MemExecutorStore<TestSpec>> {
        CommitWalker::new(store, CommitIndex::from(3), Some(CommitIndex::from(5)))
    }

    /// Accepts the links and resume nodes it's told to.
    struct SetJudge {
        acceptable: HashSet<u8>,
        resumable: HashSet<u8>,
    }

    impl SetJudge {
        fn new(acceptable: &[u8], resumable: &[u8]) -> Self {
            Self {
                acceptable: acceptable.iter().copied().collect(),
                resumable: resumable.iter().copied().collect(),
            }
        }
    }

    impl StepJudge<TestSpec> for SetJudge {
        fn check_step_acceptable(
            &mut self,
            step: &CommittedStep<TestSpec>,
        ) -> Result<bool, GExecError> {
            Ok(self.acceptable.contains(&step.lref().0))
        }

        fn check_can_resume_at(&mut self, node: &TestRef) -> Result<bool, GExecError> {
            Ok(self.resumable.contains(&node.0))
        }
    }

    #[test]
    fn test_walker_steps_back_across_segments_to_the_first() {
        let store = store();
        let mut walker = walker(&store);

        let mut seen = Vec::new();
        while let Some(step) = walker.next_step().expect("test: next step") {
            seen.push((step.lref().0, step.origin().0, step.target().0));
        }

        assert_eq!(
            seen,
            vec![(14, 5, 6), (13, 4, 5), (12, 3, 4), (11, 2, 3), (10, 1, 2),]
        );
    }

    #[test]
    fn test_find_segments_between_cuts_the_ends() {
        let store = store();

        let found = find_segments_between(walker(&store), &TestRef(2), &TestRef(5))
            .expect("test: find segments");
        assert_eq!(
            found,
            vec![
                path(2, &[(11, 3)]),
                path(3, &[(12, 4)]),
                path(4, &[(13, 5)]),
            ]
        );

        // Ends on commit boundaries leave nothing empty behind.
        let found = find_segments_between(walker(&store), &TestRef(3), &TestRef(4))
            .expect("test: find segments");
        assert_eq!(found, vec![path(3, &[(12, 4)])]);

        let found = find_segments_between(walker(&store), &TestRef(4), &TestRef(4))
            .expect("test: find segments");
        assert!(found.is_empty());

        let err = find_segments_between(walker(&store), &TestRef(9), &TestRef(5)).unwrap_err();
        assert!(matches!(err, GExecError::CorruptCommitLog(_)));
    }

    #[test]
    fn test_find_log_cut_takes_the_newest_commit_with_the_node() {
        let store = store();
        let cut = |node| find_log_cut(walker(&store), &TestRef(node)).expect("test: find cut");

        let inside = cut(5).expect("test: node in log");
        assert_eq!(inside.idx(), CommitIndex::from(5));
        assert!(!inside.is_at_start() && !inside.is_at_end());
        assert_eq!(inside.get_part_before(), path(4, &[(13, 5)]));
        assert_eq!(inside.get_part_after(), path(5, &[(14, 6)]));

        // A node two commits meet at belongs to the newer one.
        let boundary = cut(4).expect("test: node in log");
        assert_eq!(boundary.idx(), CommitIndex::from(5));
        assert!(boundary.is_at_start());

        let tip = cut(6).expect("test: node in log");
        assert!(tip.is_at_end());

        assert!(cut(9).is_none());
    }

    #[test]
    fn test_walker_stops_at_its_first_index_and_reports_gaps() {
        let store = store();

        let mut bounded =
            CommitWalker::new(&store, CommitIndex::from(5), Some(CommitIndex::from(5)));
        let segment = bounded
            .next_segment()
            .expect("test: next segment")
            .expect("test: segment");
        assert_eq!(segment.idx(), CommitIndex::from(5));
        assert_eq!(segment.into_path(), path(4, &[(13, 5), (14, 6)]));
        assert!(
            bounded
                .next_segment()
                .expect("test: next segment")
                .is_none()
        );

        let mut gapped =
            CommitWalker::new(&store, CommitIndex::from(3), Some(CommitIndex::from(6)));
        let err = expect_err(gapped.next_step(), "missing segment to be reported");
        assert!(
            matches!(err, GExecError::MissingCommitSegment(idx) if idx == CommitIndex::from(6))
        );
    }

    #[test]
    fn test_find_resume_path() {
        struct Case {
            acceptable: &'static [u8],
            resumable: &'static [u8],
            floor: u8,
            /// The node resumed at, if anything is to be redone.
            expected: Option<u8>,
        }

        const ALL_NODES: &[u8] = &[1, 2, 3, 4, 5, 6];
        let case = |acceptable, resumable, floor, expected| Case {
            acceptable,
            resumable,
            floor,
            expected,
        };
        let cases = [
            // Newest link is fine, so nothing before it is looked at.
            case(&[14], ALL_NODES, 1, None),
            // Resumes after the first acceptable link.
            case(&[10, 11, 12], ALL_NODES, 1, Some(4)),
            // An acceptable link is redone if its target can't be resumed at.
            case(&[10, 11, 12], &[3], 1, Some(3)),
            // Nothing acceptable falls back on the oldest node.
            case(&[], ALL_NODES, 1, Some(1)),
            // The floor is as far back as it goes.
            case(&[], ALL_NODES, 4, Some(4)),
            case(&[10], ALL_NODES, 6, None),
        ];

        let store = store();
        let committed = path(1, &[(10, 2), (11, 3), (12, 4), (13, 5), (14, 6)]);
        for case in cases {
            let mut judge = SetJudge::new(case.acceptable, case.resumable);
            let floor = TestRef(case.floor);
            let found = find_resume_path(walker(&store), &TestRef(6), &floor, &mut judge)
                .expect("test: find resume path");

            let resumed = match found {
                ResumeSearch::UpToDate => None,
                ResumeSearch::ResumeAt(path) => Some(path),
                ResumeSearch::NoResumePoint => panic!("test: expected somewhere to resume"),
            };
            let expected = case.expected.map(|node| {
                let pos = committed
                    .get_node_index(&TestRef(node))
                    .expect("test: node on path");
                committed.slice(pos, committed.len())
            });
            assert_eq!(resumed, expected);
        }
    }

    #[test]
    fn test_find_resume_path_reports_nowhere_to_resume() {
        let store = store();
        let mut judge = SetJudge::new(&[10], &[]);

        let found = find_resume_path(walker(&store), &TestRef(6), &TestRef(1), &mut judge)
            .expect("test: find resume path");

        assert!(matches!(found, ResumeSearch::NoResumePoint));
    }
}
