//! In-memory executor store.

use std::collections::BTreeMap;
use std::sync::Mutex;

use strata_gchain_types::*;

use crate::core::{CommitIndex, CommitTrackingState, StageTrackingState};
use crate::store::{ArtifactRecord, ExecutorStore};

/// In-memory [`ExecutorStore`], for tests and executors that don't need to
/// survive a restart.
pub struct MemExecutorStore<S: GChainSpec> {
    inner: Mutex<MemStoreInner<S>>,
}

struct MemStoreInner<S: GChainSpec> {
    artifacts: BTreeMap<(LinkRef<S>, ProcId), ProcessorArtifactData>,
    commits: Option<CommitTrackingState<S>>,
    stages: Option<StageTrackingState<S>>,
    segments: BTreeMap<CommitIndex, LinkPath<S>>,
}

impl<S: GChainSpec> MemExecutorStore<S> {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(MemStoreInner {
                artifacts: BTreeMap::new(),
                commits: None,
                stages: None,
                segments: BTreeMap::new(),
            }),
        }
    }

    fn with_inner<R>(&self, f: impl FnOnce(&mut MemStoreInner<S>) -> R) -> Result<R, BoxedError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "executor store mutex poisoned")?;
        Ok(f(&mut inner))
    }
}

impl<S: GChainSpec> Default for MemExecutorStore<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: GChainSpec> ExecutorStore for MemExecutorStore<S> {
    type Spec = S;

    fn store_commit_tracking(&self, tracking: &CommitTrackingState<S>) -> Result<(), BoxedError> {
        self.with_inner(|inner| inner.commits = Some(tracking.clone()))
    }

    fn load_commit_tracking(&self) -> Result<Option<CommitTrackingState<S>>, BoxedError> {
        self.with_inner(|inner| inner.commits.clone())
    }

    fn store_stage_tracking(&self, tracking: &StageTrackingState<S>) -> Result<(), BoxedError> {
        self.with_inner(|inner| inner.stages = Some(tracking.clone()))
    }

    fn load_stage_tracking(&self) -> Result<Option<StageTrackingState<S>>, BoxedError> {
        self.with_inner(|inner| inner.stages.clone())
    }

    fn store_commit_segment(&self, idx: CommitIndex, path: &LinkPath<S>) -> Result<(), BoxedError> {
        self.with_inner(|inner| {
            inner.segments.insert(idx, path.clone());
        })
    }

    fn load_commit_segment(&self, idx: CommitIndex) -> Result<Option<LinkPath<S>>, BoxedError> {
        self.with_inner(|inner| inner.segments.get(&idx).cloned())
    }

    fn discard_commit_segments_from(&self, idx: CommitIndex) -> Result<(), BoxedError> {
        self.with_inner(|inner| inner.segments.retain(|i, _| *i < idx))
    }

    fn discard_commit_segments_before(&self, idx: CommitIndex) -> Result<(), BoxedError> {
        self.with_inner(|inner| inner.segments.retain(|i, _| *i >= idx))
    }

    fn store_artifact(&self, record: &ArtifactRecord<S>) -> Result<(), BoxedError> {
        self.with_inner(|inner| {
            let key = (record.lref().clone(), record.proc_id());
            inner.artifacts.insert(key, record.data().clone());
        })
    }

    fn load_link_artifacts(&self, lref: &LinkRef<S>) -> Result<Vec<ArtifactRecord<S>>, BoxedError> {
        self.with_inner(|inner| {
            inner
                .artifacts
                .iter()
                .filter(|((l, _), _)| l == lref)
                .map(|((l, proc_id), data)| ArtifactRecord::new(l.clone(), *proc_id, data.clone()))
                .collect()
        })
    }

    fn has_link_artifacts(&self, lref: &LinkRef<S>) -> Result<bool, BoxedError> {
        self.with_inner(|inner| inner.artifacts.keys().any(|(l, _)| l == lref))
    }

    fn discard_link_artifacts(&self, lref: &LinkRef<S>) -> Result<(), BoxedError> {
        self.with_inner(|inner| inner.artifacts.retain(|(l, _), _| l != lref))
    }
}
