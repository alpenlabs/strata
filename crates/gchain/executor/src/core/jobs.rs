//! Stage work as the core hands it out and takes it back.
//!
//! Every call into a stage that changes something is a job the ledger
//! issues with everything the call takes, and every job yields a receipt
//! that only the ledger can take back ([`Ledger::apply_job_receipt`]).  A
//! receipt can't be made any other way: its fields are private to this
//! module, and the [`Receipt`] trait is sealed.  So the record of the
//! stages only ever moves on work that was actually done, while who runs a
//! job, where, and when, is the executor's business.
//!
//! Each job is for one stage and, where it applies, one link.  Nothing
//! here knows about a whole link or a whole path; that's strategy.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::ArtifactView;
use crate::errors::GExecResult;
use crate::store::ExecutorStore;

use super::ledger::Ledger;
use super::sealed;
use super::stage::{ProducedArtifact, Stage};

/// One call into a stage, with everything it takes.
pub(crate) trait StageJob<S: GChainSpec> {
    /// What running the job yields.
    type Receipt;

    /// Makes the call.
    fn run(self) -> GExecResult<Self::Receipt>;
}

/// Proof of stage work done, for the ledger to take back.
///
/// Sealed: only the core's own jobs produce these.
pub(crate) trait Receipt<S: GChainSpec>: sealed::Sealed {
    /// Brings the ledger up to date with the work.
    fn apply<X: ExecutorStore<Spec = S>>(self, ledger: &mut Ledger<S, X>) -> GExecResult<()>;
}

// ---- init ------------------------------------------------------------------

/// Initializing a stage's aggregated state at the committed node.
pub(crate) struct InitJob<S: GChainSpec> {
    stage: Stage<S>,
    node: NodeRef<S>,
}

impl<S: GChainSpec> InitJob<S> {
    pub(super) fn new(stage: Stage<S>, node: NodeRef<S>) -> Self {
        Self { stage, node }
    }
}

impl<S: GChainSpec> StageJob<S> for InitJob<S> {
    type Receipt = InitReceipt<S>;

    fn run(self) -> GExecResult<InitReceipt<S>> {
        self.stage.init_at(&self.node)?;
        Ok(InitReceipt {
            proc_id: self.stage.proc_id(),
            node: self.node,
        })
    }
}

/// A stage was initialized at a node.
pub(crate) struct InitReceipt<S: GChainSpec> {
    proc_id: ProcId,
    node: NodeRef<S>,
}

impl<S: GChainSpec> sealed::Sealed for InitReceipt<S> {}

impl<S: GChainSpec> Receipt<S> for InitReceipt<S> {
    fn apply<X: ExecutorStore<Spec = S>>(self, ledger: &mut Ledger<S, X>) -> GExecResult<()> {
        ledger.record_stage_inited(self.proc_id, self.node)
    }
}

// ---- process ---------------------------------------------------------------

/// Running a stage on a link whose pre-state is reached by a path.
///
/// Borrows the artifacts it draws on from the ledger that issued it, so it
/// runs on the thread the ledger is on; an executor that runs jobs elsewhere
/// would issue one over an owned snapshot instead.
pub(crate) struct ProcessJob<'r, S: GChainSpec, V: ArtifactView<S>> {
    stage: &'r Stage<S>,
    lref: &'r LinkRef<S>,
    link: &'r Link<S>,
    path: &'r LinkPath<S>,
    artifacts: &'r V,
}

impl<'r, S: GChainSpec, V: ArtifactView<S>> ProcessJob<'r, S, V> {
    pub(super) fn new(
        stage: &'r Stage<S>,
        lref: &'r LinkRef<S>,
        link: &'r Link<S>,
        path: &'r LinkPath<S>,
        artifacts: &'r V,
    ) -> Self {
        Self {
            stage,
            lref,
            link,
            path,
            artifacts,
        }
    }
}

impl<S: GChainSpec, V: ArtifactView<S>> StageJob<S> for ProcessJob<'_, S, V> {
    type Receipt = ProcessReceipt<S>;

    fn run(self) -> GExecResult<ProcessReceipt<S>> {
        let produced = self
            .stage
            .process(self.lref, self.link, self.artifacts, self.path)?;
        Ok(ProcessReceipt { produced })
    }
}

/// A stage was run on a link and made an artifact of it, which may say the
/// link is invalid.
pub(crate) struct ProcessReceipt<S: GChainSpec> {
    produced: ProducedArtifact<S>,
}

impl<S: GChainSpec> ProcessReceipt<S> {
    /// Whether the stage found the link valid.  An executor that stops at
    /// the first stage that didn't reads this before applying the receipt.
    pub(crate) fn is_link_valid(&self) -> bool {
        self.produced.is_link_valid()
    }
}

impl<S: GChainSpec> sealed::Sealed for ProcessReceipt<S> {}

impl<S: GChainSpec> Receipt<S> for ProcessReceipt<S> {
    fn apply<X: ExecutorStore<Spec = S>>(self, ledger: &mut Ledger<S, X>) -> GExecResult<()> {
        ledger.record_artifact(self.produced)
    }
}

// ---- commit ----------------------------------------------------------------

/// Committing one commit segment in a stage, with the artifacts it takes.
pub(crate) struct CommitJob<S: GChainSpec> {
    stage: Stage<S>,
    segment: LinkPath<S>,
    artifacts: Vec<Arc<dyn DynProcArtifact>>,
}

impl<S: GChainSpec> CommitJob<S> {
    pub(super) fn new(
        stage: Stage<S>,
        segment: LinkPath<S>,
        artifacts: Vec<Arc<dyn DynProcArtifact>>,
    ) -> Self {
        Self {
            stage,
            segment,
            artifacts,
        }
    }
}

impl<S: GChainSpec> StageJob<S> for CommitJob<S> {
    type Receipt = CommitReceipt<S>;

    fn run(self) -> GExecResult<CommitReceipt<S>> {
        self.stage.commit(&self.segment, &self.artifacts)?;
        Ok(CommitReceipt {
            proc_id: self.stage.proc_id(),
            from: self.segment.base_node().clone(),
            to: self.segment.terminal_node().clone(),
        })
    }
}

/// A stage committed a segment, moving it forward.
pub(crate) struct CommitReceipt<S: GChainSpec> {
    proc_id: ProcId,
    from: NodeRef<S>,
    to: NodeRef<S>,
}

impl<S: GChainSpec> sealed::Sealed for CommitReceipt<S> {}

impl<S: GChainSpec> Receipt<S> for CommitReceipt<S> {
    fn apply<X: ExecutorStore<Spec = S>>(self, ledger: &mut Ledger<S, X>) -> GExecResult<()> {
        ledger.record_stage_committed(self.proc_id, &self.from, self.to)
    }
}

// ---- uncommit --------------------------------------------------------------

/// Undoing one commit segment in a stage, with the artifacts it committed it
/// with.
///
/// The stage is recorded at the start of the segment before this is handed
/// out, so dropping it unrun only leaves the stage recorded behind where it
/// is, which is caught up like any lagging stage.
pub(crate) struct UncommitJob<S: GChainSpec> {
    stage: Stage<S>,
    segment: LinkPath<S>,
    artifacts: Vec<Arc<dyn DynProcArtifact>>,
}

impl<S: GChainSpec> UncommitJob<S> {
    pub(super) fn new(
        stage: Stage<S>,
        segment: LinkPath<S>,
        artifacts: Vec<Arc<dyn DynProcArtifact>>,
    ) -> Self {
        Self {
            stage,
            segment,
            artifacts,
        }
    }
}

impl<S: GChainSpec> StageJob<S> for UncommitJob<S> {
    type Receipt = UncommitReceipt<S>;

    fn run(self) -> GExecResult<UncommitReceipt<S>> {
        self.stage.uncommit(&self.segment, &self.artifacts)?;
        Ok(UncommitReceipt {
            proc_id: self.stage.proc_id(),
            node: self.segment.base_node().clone(),
        })
    }
}

/// A stage undid a segment, and is now where it was already recorded.
pub(crate) struct UncommitReceipt<S: GChainSpec> {
    proc_id: ProcId,
    node: NodeRef<S>,
}

impl<S: GChainSpec> sealed::Sealed for UncommitReceipt<S> {}

impl<S: GChainSpec> Receipt<S> for UncommitReceipt<S> {
    fn apply<X: ExecutorStore<Spec = S>>(self, ledger: &mut Ledger<S, X>) -> GExecResult<()> {
        ledger.record_stage_uncommitted(self.proc_id, &self.node);
        Ok(())
    }
}

// ---- prune -----------------------------------------------------------------

/// Letting a stage discard what it kept for rolling back to before a node.
pub(crate) struct PruneJob<S: GChainSpec> {
    stage: Stage<S>,
    node: NodeRef<S>,
}

impl<S: GChainSpec> PruneJob<S> {
    pub(super) fn new(stage: Stage<S>, node: NodeRef<S>) -> Self {
        Self { stage, node }
    }
}

impl<S: GChainSpec> StageJob<S> for PruneJob<S> {
    type Receipt = PruneReceipt;

    fn run(self) -> GExecResult<PruneReceipt> {
        self.stage.prune_upto(&self.node)?;
        Ok(PruneReceipt(()))
    }
}

/// A stage discarded what it kept for rolling back.
///
/// The core keeps no record of how far a stage has pruned, so there's
/// nothing to bring up to date; the receipt only closes the job.
pub(crate) struct PruneReceipt(());

impl sealed::Sealed for PruneReceipt {}

impl<S: GChainSpec> Receipt<S> for PruneReceipt {
    fn apply<X: ExecutorStore<Spec = S>>(self, _ledger: &mut Ledger<S, X>) -> GExecResult<()> {
        Ok(())
    }
}
