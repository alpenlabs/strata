//! Describing a pipeline of processor stages to an executor.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::core::*;
use crate::errors::GExecError;
use crate::open::*;
use crate::reprocess::*;
use crate::schedule::{StageSchedule, StageScheduleBuilder};
use crate::stage::Stage;
use crate::store::ExecutorStore;

/// Pipeline of processor stages in canonical order with their schedule.
///
/// The stages are held in canonical order and the schedule only indexes into
/// them, so there's no second copy of the ordering that could disagree with
/// the first.  Built with [`PipelineBuilder`].
pub(crate) struct StagePipeline<S: GChainSpec> {
    stages: Vec<Stage<S>>,
    schedule: StageSchedule,
}

impl<S: GChainSpec> StagePipeline<S> {
    /// Returns an iterator over the stages in canonical order.
    pub(crate) fn iter_stages(&self) -> impl DoubleEndedIterator<Item = &Stage<S>> {
        self.stages.iter()
    }

    /// The stages' IDs in canonical order.
    pub(crate) fn proc_ids(&self) -> impl DoubleEndedIterator<Item = ProcId> {
        self.stages.iter().map(Stage::proc_id)
    }

    /// Looks up a stage by the ID it's registered under.
    pub(crate) fn get_stage(&self, proc_id: ProcId) -> Option<&Stage<S>> {
        self.schedule.index_of(proc_id).map(|idx| &self.stages[idx])
    }

    pub(crate) fn schedule(&self) -> &StageSchedule {
        &self.schedule
    }
}

/// Assembles a [`StagePipeline`] one stage at a time, in canonical order.
///
/// This is what pairs a stage with the ID it's registered under, so the two
/// can't drift apart.  Deps are validated as each stage is added: IDs must be
/// unique, and a dep must name a stage already added (or, for a path dep, the
/// stage itself).
pub(crate) struct PipelineBuilder<S: GChainSpec> {
    stages: Vec<Stage<S>>,
    schedule: StageScheduleBuilder,
}

impl<S: GChainSpec> PipelineBuilder<S> {
    pub(crate) fn new() -> Self {
        Self {
            stages: Vec::new(),
            schedule: StageScheduleBuilder::new(),
        }
    }

    /// Appends a stage under an ID, after every stage added so far.
    pub(crate) fn add_stage(
        mut self,
        proc_id: ProcId,
        proc: impl GChainProc<Spec = S>,
        deps: ProcDeps,
    ) -> Result<Self, GExecError> {
        self.schedule.add_stage(proc_id, deps)?;
        self.stages.push(Stage::new(proc_id, proc));
        Ok(self)
    }

    pub(crate) fn build(self) -> StagePipeline<S> {
        StagePipeline {
            stages: self.stages,
            schedule: self.schedule.build(),
        }
    }
}

/// Assembles a [`crate::SerialExecutor`] over a provider and a store, one processor stage at
/// a time in canonical order.
pub struct SerialExecutorBuilder<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
> {
    pipeline: PipelineBuilder<S>,
    provider: Arc<P>,
    store: Arc<X>,
    base_node: NodeRef<S>,
}

impl<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>
    SerialExecutorBuilder<S, P, X>
{
    /// Starts an executor with no stages.
    ///
    /// A store that has never been used starts every stage at `base_node`,
    /// which is then the oldest node the pipeline can roll back to until it's
    /// pruned past.
    pub fn new(provider: Arc<P>, store: Arc<X>, base_node: NodeRef<S>) -> Self {
        Self {
            pipeline: PipelineBuilder::new(),
            provider,
            store,
            base_node,
        }
    }

    /// Appends a stage under an ID, after every stage added so far.
    ///
    /// IDs must be unique, and a dep must name a stage already added (or, for
    /// a path dep, the stage itself).
    pub fn add_stage(
        mut self,
        proc_id: ProcId,
        proc: impl GChainProc<Spec = S>,
        deps: ProcDeps,
    ) -> Result<Self, GExecError> {
        self.pipeline = self.pipeline.add_stage(proc_id, proc, deps)?;
        Ok(self)
    }

    /// Opens the executor over the store, reconciling what it holds with the
    /// stages.
    ///
    /// The committed path's artifacts are loaded.  If the stages accept them
    /// all, stages that have no committed state or lag the committed node are
    /// brought level and the executor is ready.  If not, what's handed back
    /// is the means to reprocess the links in question, which gives the
    /// executor once that's done.
    ///
    /// Uncommitted links are left where they are; one whose artifacts turn
    /// out to be missing or no longer accepted has those stages re-run when
    /// it's next processed.
    pub fn open(self) -> Result<OpenResult<S, P, X>, GExecError> {
        let pipeline = self.pipeline.build();
        let core = ExecutorCore::open(pipeline, self.provider, self.store, self.base_node)?;
        let plans = plan_reprocessing(&core)?;
        if !plans.is_empty() {
            let pending = ReprocExecutor::new(core, plans);
            return Ok(OpenResult::NeedsReprocess(pending));
        }

        let (exec, report) = finish_open(core, ReprocLog::new(Vec::new()))?;
        Ok(OpenResult::Ready(exec, report))
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

    fn no_deps() -> ProcDeps {
        ProcDeps::new(Vec::new(), Vec::new())
    }

    /// The builder is what pairs each stage with its ID and position, and it
    /// surfaces the schedule's dep checks (which have their own tests).
    #[test]
    fn test_builder_registers_stages_in_order_and_checks_deps() {
        let pipeline = PipelineBuilder::<TestSpec>::new()
            .add_stage(id("third"), TestProc::new(), no_deps())
            .expect("test: add stage")
            .add_stage(id("first"), TestProc::new(), no_deps())
            .expect("test: add stage")
            .build();

        let ids: Vec<_> = pipeline.proc_ids().collect();
        assert_eq!(ids, vec![id("third"), id("first")]);
        let found = pipeline.get_stage(id("first")).expect("test: find stage");
        assert_eq!(found.proc_id(), id("first"));
        assert!(pipeline.get_stage(id("absent")).is_none());

        let err = expect_err(
            PipelineBuilder::<TestSpec>::new().add_stage(
                id("a"),
                TestProc::new(),
                ProcDeps::new(vec![id("b")], Vec::new()),
            ),
            "forward dep to be rejected",
        );
        assert!(matches!(err, GExecError::DepNotRegistered { .. }));
    }
}
