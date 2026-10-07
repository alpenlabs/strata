//! The artifacts the pipeline's stages stand behind, as the stages judge
//! them.
//!
//! An [`ArtifactCache`] only knows what's there; this knows what counts.
//! Pairing the cache with the pipeline is what decides whether a stored
//! artifact is the stage's current one for its link, which stages a link is
//! missing, and which stages' deps a job can draw on.  It does no store I/O:
//! the ledger feeds it the stored artifacts a link has, and records with it
//! the artifacts stages produce once they're persisted.
//!
//! An artifact that says its link is invalid is kept like any other: it's
//! the stage's finding.  But it never *covers* the link, so a stage that
//! depends on it can't run on the link, no path can run through the link,
//! and the link can't be committed.
//!
//! Nothing here calls into a stage to change it, bar letting a stage clean
//! up after an artifact that's being forgotten.

use std::collections::HashSet;
use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::{ArtifactCache, ArtifactView};
use crate::errors::{GExecError, GExecResult};
use crate::store::ArtifactRecord;

use super::pipeline::StagePipeline;
use super::schedule::LinkCoverage;
use super::stage::{ProducedArtifact, Stage};

/// Whether an artifact counts as covering its link: it's there, and it
/// doesn't say the link is invalid.
fn covers_link(artifact: Option<&Arc<dyn DynProcArtifact>>) -> bool {
    artifact.is_some_and(|a| a.is_link_valid())
}

pub(crate) struct StageArtifacts<S: GChainSpec> {
    pipeline: StagePipeline<S>,
    cache: ArtifactCache<S>,
}

impl<S: GChainSpec> StageArtifacts<S> {
    pub(super) fn new(pipeline: StagePipeline<S>) -> Self {
        Self {
            pipeline,
            cache: ArtifactCache::new(),
        }
    }

    pub(super) fn pipeline(&self) -> &StagePipeline<S> {
        &self.pipeline
    }

    /// The artifacts the stages stand behind, for the links loaded.
    pub(super) fn cache(&self) -> &ArtifactCache<S> {
        &self.cache
    }

    /// Looks up a stage by the ID it's registered under.
    ///
    /// # Panics
    ///
    /// If no stage is registered under the ID, since an executor only asks
    /// about stages it registered.
    pub(super) fn get_stage(&self, proc_id: ProcId) -> &Stage<S> {
        self.pipeline
            .get_stage(proc_id)
            .expect("gchain: proc ID doesn't point to stage")
    }

    pub(super) fn get_artifact<A: ProcArtifact>(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<Arc<A>> {
        self.cache.get_artifact(lref, proc_id)
    }

    /// Whether taking in a stored artifact needs its link's header, which is
    /// what a stage judges an older version's artifact by.
    pub(super) fn check_needs_header(&self, record: &ArtifactRecord<S>) -> bool {
        self.pipeline
            .get_stage(record.proc_id())
            .is_some_and(|stage| record.data().exec_version() < stage.proc_version())
    }

    /// Takes in a stored artifact from a stage still in the pipeline.
    ///
    /// It counts as the stage's artifact for the link if it came from the
    /// stage's current version or an older one the stage still accepts for
    /// the link, and is passed over otherwise.  Without the header an older
    /// version's artifact is never accepted.
    pub(super) fn insert_stored(
        &mut self,
        record: ArtifactRecord<S>,
        header: Option<&LinkHeader<S>>,
    ) -> GExecResult<()> {
        let (lref, proc_id, data) = record.into_parts();
        let Some(stage) = self.pipeline.get_stage(proc_id) else {
            return Ok(());
        };

        let stored = data.exec_version();
        let current = stage.proc_version();
        if stored > current {
            return Err(GExecError::artifact_from_newer_version(&lref, proc_id));
        }

        let acceptable = stored == current
            || header.is_some_and(|h| stage.check_artifact_acceptable(stored, &lref, h));
        if acceptable {
            let artifact = stage.decode(&data)?;
            self.cache.insert_artifact(lref, proc_id, artifact);
        }
        Ok(())
    }

    /// Puts an artifact a stage produced on record as the stage's artifact
    /// for its link.
    pub(super) fn insert_produced(&mut self, produced: ProducedArtifact<S>) {
        let (artifact, record) = produced.into_parts();
        let (lref, proc_id, _) = record.into_parts();
        self.cache.insert_artifact(lref, proc_id, artifact);
    }

    /// The stages that don't cover a link, in canonical order.  Every stage
    /// when nothing is cached for it.
    pub(super) fn missing_stages(&self, lref: &LinkRef<S>) -> Vec<ProcId> {
        self.pipeline
            .proc_ids()
            .filter(|proc_id| !self.covers(lref, *proc_id))
            .collect()
    }

    /// Whether a stage covers a link: it has a current artifact for it that
    /// doesn't say the link is invalid.
    pub(super) fn covers(&self, lref: &LinkRef<S>, proc_id: ProcId) -> bool {
        covers_link(self.cache.get_artifact_dyn(lref, proc_id))
    }

    /// Drops the cached artifacts of every link outside a set.
    pub(super) fn retain_links(&mut self, keep: &HashSet<LinkRef<S>>) {
        self.cache.retain_links(keep);
    }

    /// Lets each stage clean up after its artifact for a link being
    /// forgotten, then drops the cached artifacts.
    pub(super) fn discard(&mut self, lref: &LinkRef<S>) -> GExecResult<()> {
        for stage in self.pipeline.iter_stages() {
            if let Some(artifact) = self.cache.get_artifact_dyn(lref, stage.proc_id()) {
                stage.preprune(lref, artifact.as_ref())?;
            }
        }

        self.cache.remove_link(lref);
        Ok(())
    }

    /// Checks that every stage could commit a path, which needs it to cover
    /// every link on it.
    pub(super) fn check_committable(&self, path: &LinkPath<S>) -> GExecResult<()> {
        self.find_uncovered(path).map_or(Ok(()), |(lref, proc_id)| {
            Err(GExecError::missing_artifact(lref, proc_id))
        })
    }

    /// Checks that every stage could undo a committed path, which needs it
    /// to cover every link on it.  A stage added after a link was committed
    /// has nothing for it.
    pub(super) fn check_undoable(&self, path: &LinkPath<S>) -> GExecResult<()> {
        self.find_uncovered(path).map_or(Ok(()), |(lref, proc_id)| {
            Err(GExecError::stale_artifact(lref, proc_id))
        })
    }

    /// Works out which stages cover a link and the path to its origin.
    pub(super) fn get_link_coverage(&self, lref: &LinkRef<S>, path: &LinkPath<S>) -> LinkCoverage {
        let mut coverage = LinkCoverage::new();
        for proc_id in self.pipeline.proc_ids() {
            if self.covers(lref, proc_id) {
                coverage.mark_cur(proc_id);
            }
            if path.links().iter().all(|l| self.covers(l, proc_id)) {
                coverage.mark_path(proc_id);
            }
        }
        coverage
    }

    /// The first link on a path some stage doesn't cover, with the stage,
    /// taking the stages in canonical order.
    fn find_uncovered<'p>(&self, path: &'p LinkPath<S>) -> Option<(&'p LinkRef<S>, ProcId)> {
        self.pipeline.proc_ids().find_map(|proc_id| {
            path.links()
                .iter()
                .find(|lref| !self.covers(lref, proc_id))
                .map(|lref| (lref, proc_id))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::test_support::*;

    fn id(s: &str) -> ProcId {
        ProcId::from_str(s).expect("test: parse ProcId")
    }

    fn ids(names: &[&str]) -> Vec<ProcId> {
        names.iter().map(|n| id(n)).collect()
    }

    /// A path from a base node through links given as `(lref, target)`.
    fn path(base: u8, steps: &[(u8, u8)]) -> LinkPath<TestSpec> {
        LinkPath::from_steps(
            TestRef(base),
            steps.iter().map(|(l, n)| (TestRef(*l), TestRef(*n))),
        )
    }

    fn record(lref: u8, proc: &str, version: u32) -> ArtifactRecord<TestSpec> {
        let data =
            ProcessorArtifactData::from_artifact(ProcVersion::from(version), &FlagArtifact(true))
                .expect("test: encode artifact");
        ArtifactRecord::new(TestRef(lref), id(proc), data)
    }

    fn artifacts(procs: Vec<TestProc>) -> StageArtifacts<TestSpec> {
        StageArtifacts::new(pipeline_of(procs))
    }

    #[test]
    fn test_stored_artifacts_count_only_at_the_current_version() {
        let mut artifacts = artifacts(vec![TestProc::new(), TestProc::new()]);
        assert_eq!(artifacts.missing_stages(&TestRef(10)), ids(&["a", "b"]));

        for rec in [record(10, "a", 1), record(12, "gone", 1)] {
            artifacts
                .insert_stored(rec, None)
                .expect("test: insert stored");
        }

        assert_eq!(artifacts.missing_stages(&TestRef(10)), ids(&["b"]));
        assert_eq!(artifacts.missing_stages(&TestRef(12)), ids(&["a", "b"]));

        let err = artifacts
            .insert_stored(record(11, "a", 2), None)
            .unwrap_err();
        assert!(matches!(err, GExecError::ArtifactFromNewerVersion { .. }));
    }

    #[test]
    fn test_older_artifacts_count_where_the_stage_accepts_them() {
        let proc = TestProc::new().with_version(2).accepting_old([10]);
        let mut artifacts = artifacts(vec![proc]);

        for lref in [10, 11] {
            assert!(artifacts.check_needs_header(&record(lref, "a", 1)));
            artifacts
                .insert_stored(record(lref, "a", 1), Some(&TestLink(lref)))
                .expect("test: insert stored");
        }
        assert!(!artifacts.check_needs_header(&record(12, "a", 2)));

        assert!(artifacts.missing_stages(&TestRef(10)).is_empty());
        assert_eq!(artifacts.missing_stages(&TestRef(11)), ids(&["a"]));
    }

    /// A stage can only commit or undo a link it has an artifact for, and an
    /// artifact it no longer accepts isn't one.
    #[test]
    fn test_commit_and_undo_need_a_cached_artifact() {
        let first = TestProc::new().with_version(2);
        let second = TestProc::new();
        let mut artifacts = artifacts(vec![first, second]);
        let path = path(1, &[(10, 2)]);
        for rec in [record(10, "a", 1), record(10, "b", 1)] {
            artifacts
                .insert_stored(rec, Some(&TestLink(10)))
                .expect("test: insert stored");
        }

        assert_eq!(artifacts.missing_stages(&TestRef(10)), ids(&["a"]));
        let err = artifacts.check_undoable(&path).unwrap_err();
        assert!(matches!(err, GExecError::StaleArtifact { proc_id, .. } if proc_id == id("a")));
        let err = artifacts.check_committable(&path).unwrap_err();
        assert!(matches!(err, GExecError::MissingArtifact { proc_id, .. } if proc_id == id("a")));

        artifacts
            .insert_stored(record(10, "a", 2), None)
            .expect("test: insert stored");
        artifacts.check_undoable(&path).expect("test: undoable");
        artifacts
            .check_committable(&path)
            .expect("test: committable");
    }

    #[test]
    fn test_discard_only_prepunes_cached_artifacts() {
        let first = TestProc::new();
        let second = TestProc::new();
        let (first_events, second_events) = (first.events(), second.events());
        let mut artifacts = artifacts(vec![first, second]);
        artifacts
            .insert_stored(record(10, "a", 1), None)
            .expect("test: insert stored");

        artifacts.discard(&TestRef(10)).expect("test: discard");

        assert_eq!(
            take_events(&first_events),
            vec![ProcEvent::Preprune(TestRef(10))]
        );
        assert!(take_events(&second_events).is_empty());
        assert_eq!(artifacts.missing_stages(&TestRef(10)), ids(&["a", "b"]));
    }
}
