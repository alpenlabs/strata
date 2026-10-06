//! One processor stage as an executor drives it.
//!
//! This is the lowest level the executor talks to a stage at: each call here
//! is one call into the stage, with its errors tagged by the stage they came
//! from.  Which artifacts a stage is handed, and what's done with the ones it
//! produces, is up to the caller.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::ArtifactCache;
use crate::errors::GExecError;
use crate::process::{GChainProcDyn, ProcShim};
use crate::store::ArtifactRecord;

/// What a stage made of a link it was run on.
pub(crate) enum StageVerdict<S: GChainSpec> {
    /// The stage accepted the link, with the artifact it produced and the
    /// record of it to persist.
    Accepted {
        artifact: Arc<dyn DynProcArtifact>,
        record: ArtifactRecord<S>,
    },

    /// The stage rejected the link.
    Rejected,
}

/// A processor stage as registered in a pipeline.
pub(crate) struct Stage<S: GChainSpec> {
    chain_proc: Arc<dyn GChainProcDyn<S>>,
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
    pub(crate) fn check_can_resume_at(&self, node: &NodeRef<S>) -> Result<bool, GExecError> {
        self.chain_proc
            .check_can_resume_at(node)
            .map_err(|e| self.proc_error(e))
    }

    /// Initializes the stage's aggregated state at a node.
    pub(crate) fn init_at(&self, node: &NodeRef<S>) -> Result<(), GExecError> {
        self.chain_proc
            .on_init(node)
            .map_err(|e| self.proc_error(e))
    }

    /// Runs the stage on a link whose pre-state is reached by a path, with
    /// its deps' artifacts resolved from a cache.
    pub(crate) fn process(
        &self,
        lref: &LinkRef<S>,
        link: &Link<S>,
        cache: &ArtifactCache<S>,
        path: &LinkPath<S>,
    ) -> Result<StageVerdict<S>, GExecError> {
        let artifact = self
            .chain_proc
            .process_link(lref, link, cache, path)
            .map_err(|e| self.proc_error(e))?;
        if !artifact.is_link_valid() {
            return Ok(StageVerdict::Rejected);
        }

        let encoded = artifact.to_buf_dyn().map_err(|e| self.proc_error(e))?;
        let data = ProcessorArtifactData::new(self.proc_version(), encoded);
        let record = ArtifactRecord::new(lref.clone(), self.proc_id(), data);
        Ok(StageVerdict::Accepted { artifact, record })
    }

    /// Decodes an artifact the stage stored, whichever version of it that
    /// was.
    pub(crate) fn decode(
        &self,
        data: &ProcessorArtifactData,
    ) -> Result<Arc<dyn DynProcArtifact>, GExecError> {
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
    ) -> Result<(), GExecError> {
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
    ) -> Result<(), GExecError> {
        self.chain_proc
            .uncommit_outputs(path, artifacts)
            .map_err(|e| self.proc_error(e))
    }

    /// Lets the stage clean up after its artifact for a link being forgotten.
    pub(crate) fn preprune(
        &self,
        lref: &LinkRef<S>,
        artifact: &dyn DynProcArtifact,
    ) -> Result<(), GExecError> {
        self.chain_proc
            .preprune_artifact(lref, artifact)
            .map_err(|e| self.proc_error(e))
    }

    /// Lets the stage discard what it kept for rolling back to before a node.
    pub(crate) fn prune_upto(&self, node: &NodeRef<S>) -> Result<(), GExecError> {
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
    use crate::test_support::*;

    fn stage(proc: TestProc) -> Stage<TestSpec> {
        Stage::new(ProcId::from_str("a").expect("test: parse ProcId"), proc)
    }

    #[test]
    fn test_process_records_accepted_links_at_the_stage_version() {
        let stage = stage(TestProc::new().with_version(3).rejecting([11]));
        let cache = ArtifactCache::new();
        let path = LinkPath::new_at(TestRef(1));

        let verdict = stage
            .process(&TestRef(10), &TestLink(10), &cache, &path)
            .expect("test: process");
        let StageVerdict::Accepted { record, .. } = verdict else {
            panic!("test: link 10 should be accepted");
        };
        assert_eq!(record.lref(), &TestRef(10));
        assert_eq!(record.proc_id(), stage.proc_id());
        assert_eq!(record.data().exec_version(), ProcVersion::from(3));
        stage
            .decode(record.data())
            .expect("test: decode what was encoded");

        let verdict = stage
            .process(&TestRef(11), &TestLink(11), &cache, &path)
            .expect("test: process");
        assert!(matches!(verdict, StageVerdict::Rejected));
    }
}
