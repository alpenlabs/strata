//! One processor stage as the core drives it.
//!
//! This is the lowest level the executor talks to a stage at: each call here
//! is one call into the stage, with its errors tagged by the stage they came
//! from.  Which artifacts a stage is handed, and what's done with the ones it
//! produces, is up to the caller, which is the core's jobs for anything that
//! changes the stage's state.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::ArtifactView;
use crate::errors::{GExecError, GExecResult};
use crate::store::ArtifactRecord;

use super::process::{GChainProcDyn, ProcShim};

/// An artifact a stage produced for a link, with the record of it to
/// persist.
///
/// The artifact may say the link is invalid.  That's still the stage's
/// finding about the link and is kept like any other; it only stops stages
/// that depend on this one from running on the link.
pub(crate) struct ProducedArtifact<S: GChainSpec> {
    artifact: Arc<dyn DynProcArtifact>,
    record: ArtifactRecord<S>,
}

impl<S: GChainSpec> ProducedArtifact<S> {
    pub(crate) fn lref(&self) -> &LinkRef<S> {
        self.record.lref()
    }

    /// See [`ProcArtifact::is_link_valid`].
    pub(crate) fn is_link_valid(&self) -> bool {
        self.artifact.is_link_valid()
    }

    pub(crate) fn record(&self) -> &ArtifactRecord<S> {
        &self.record
    }

    pub(crate) fn into_parts(self) -> (Arc<dyn DynProcArtifact>, ArtifactRecord<S>) {
        (self.artifact, self.record)
    }
}

/// A processor stage as registered in a pipeline.
pub(crate) struct Stage<S: GChainSpec> {
    chain_proc: Arc<dyn GChainProcDyn<S>>,
}

// Bounds fall on the shared stage, not the marker spec type.
impl<S: GChainSpec> Clone for Stage<S> {
    fn clone(&self) -> Self {
        Self {
            chain_proc: self.chain_proc.clone(),
        }
    }
}

impl<S: GChainSpec> Stage<S> {
    /// Wraps a processor stage under the ID it's registered as.
    pub(crate) fn new(proc_id: ProcId, proc: impl GChainProc<Spec = S>) -> Self {
        Self {
            chain_proc: Arc::new(ProcShim::new(proc_id, proc)),
        }
    }

    pub(crate) fn proc_id(&self) -> ProcId {
        self.chain_proc.proc_id()
    }

    pub(crate) fn proc_version(&self) -> ProcVersion {
        self.chain_proc.proc_version()
    }

    /// See [`GChainProc::check_artifact_acceptable`].
    pub(crate) fn check_artifact_acceptable(
        &self,
        stored: ProcVersion,
        lref: &LinkRef<S>,
        header: &LinkHeader<S>,
    ) -> bool {
        self.chain_proc
            .check_artifact_acceptable(stored, lref, header)
    }

    /// Asks the stage whether it can pick up processing from a node.
    pub(crate) fn check_can_resume_at(&self, node: &NodeRef<S>) -> GExecResult<bool> {
        self.chain_proc
            .check_can_resume_at(node)
            .map_err(|e| self.proc_error(e))
    }

    /// Initializes the stage's aggregated state at a node.
    pub(crate) fn init_at(&self, node: &NodeRef<S>) -> GExecResult<()> {
        self.chain_proc
            .on_init(node)
            .map_err(|e| self.proc_error(e))
    }

    /// Runs the stage on a link whose pre-state is reached by a path, with
    /// its deps' artifacts resolved from a view of the ones on hand.
    ///
    /// Nothing but the stage is touched, so what's done with the artifact is
    /// up to the caller.
    pub(crate) fn process(
        &self,
        lref: &LinkRef<S>,
        link: &Link<S>,
        artifacts: &impl ArtifactView<S>,
        path: &LinkPath<S>,
    ) -> GExecResult<ProducedArtifact<S>> {
        let artifact = self
            .chain_proc
            .process_link(lref, link, artifacts, path)
            .map_err(|e| self.proc_error(e))?;

        let encoded = artifact.to_buf_dyn().map_err(|e| self.proc_error(e))?;
        let data = ProcessorArtifactData::new(self.proc_version(), encoded);
        let record = ArtifactRecord::new(lref.clone(), self.proc_id(), data);
        Ok(ProducedArtifact { artifact, record })
    }

    /// Decodes an artifact the stage stored, whichever version of it that
    /// was.
    pub(crate) fn decode(
        &self,
        data: &ProcessorArtifactData,
    ) -> GExecResult<Arc<dyn DynProcArtifact>> {
        self.chain_proc
            .decode_artifact(data)
            .map_err(|e| self.proc_error(e))
    }

    /// Commits a path in the stage with its artifacts for the path's links,
    /// in order.
    pub(crate) fn commit(
        &self,
        path: &LinkPath<S>,
        artifacts: &[Arc<dyn DynProcArtifact>],
    ) -> GExecResult<()> {
        self.chain_proc
            .commit_outputs(path, artifacts)
            .map_err(|e| self.proc_error(e))
    }

    /// Undoes a committed path in the stage with the artifacts that
    /// committed its links, in order.
    pub(crate) fn uncommit(
        &self,
        path: &LinkPath<S>,
        artifacts: &[Arc<dyn DynProcArtifact>],
    ) -> GExecResult<()> {
        self.chain_proc
            .uncommit_outputs(path, artifacts)
            .map_err(|e| self.proc_error(e))
    }

    /// Lets the stage clean up after its artifact for a link being forgotten.
    pub(crate) fn preprune(
        &self,
        lref: &LinkRef<S>,
        artifact: &dyn DynProcArtifact,
    ) -> GExecResult<()> {
        self.chain_proc
            .preprune_artifact(lref, artifact)
            .map_err(|e| self.proc_error(e))
    }

    /// Lets the stage discard what it kept for rolling back to before a node.
    pub(crate) fn prune_upto(&self, node: &NodeRef<S>) -> GExecResult<()> {
        self.chain_proc
            .prune_state_upto(node)
            .map_err(|e| self.proc_error(e))
    }

    fn proc_error(&self, err: ProcError) -> GExecError {
        GExecError::Proc(self.proc_id(), err)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::artifact_cache::ArtifactCache;
    use crate::test_support::*;

    fn stage(proc: TestProc) -> Stage<TestSpec> {
        Stage::new(ProcId::from_str("a").expect("test: parse ProcId"), proc)
    }

    #[test]
    fn test_process_records_links_at_the_stage_version() {
        let stage = stage(TestProc::new().with_version(3).rejecting([11]));
        let cache = ArtifactCache::new();
        let path = LinkPath::new_at(TestRef(1));

        let produced = stage
            .process(&TestRef(10), &TestLink(10), &cache, &path)
            .expect("test: process");
        assert!(produced.is_link_valid());
        let record = produced.record();
        assert_eq!(record.lref(), &TestRef(10));
        assert_eq!(record.proc_id(), stage.proc_id());
        assert_eq!(record.data().exec_version(), ProcVersion::from(3));
        stage
            .decode(record.data())
            .expect("test: decode what was encoded");

        let produced = stage
            .process(&TestRef(11), &TestLink(11), &cache, &path)
            .expect("test: process");
        assert!(!produced.is_link_valid());
        assert_eq!(produced.lref(), &TestRef(11));
    }
}
