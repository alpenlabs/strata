//! The processor stages an executor drives, in canonical order.

use std::sync::Arc;

use strata_gchain_types::*;

use super::artifacts::RecordIntake;
use super::schedule::{StageSchedule, StageScheduleBuilder};
use super::stage::Stage;
use crate::errors::{GExecError, GExecResult};
use crate::store::ArtifactRecord;

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

/// A stored record counts as its stage's artifact for the link if it came
/// from the stage's current version or an older one the stage still accepts
/// for the link, which the stage judges by the link's header.  Records from
/// stages no longer in the pipeline are passed over.
impl<S: GChainSpec> RecordIntake<S> for StagePipeline<S> {
    fn check_needs_header(&self, record: &ArtifactRecord<S>) -> bool {
        self.get_stage(record.proc_id())
            .is_some_and(|stage| record.data().exec_version() < stage.proc_version())
    }

    fn take_in(
        &self,
        record: &ArtifactRecord<S>,
        header: Option<&LinkHeader<S>>,
    ) -> GExecResult<Option<Arc<dyn DynProcArtifact>>> {
        let (lref, proc_id) = (record.lref(), record.proc_id());
        let Some(stage) = self.get_stage(proc_id) else {
            return Ok(None);
        };

        let stored = record.data().exec_version();
        let current = stage.proc_version();
        if stored > current {
            return Err(GExecError::artifact_from_newer_version(lref, proc_id));
        }

        let acceptable = stored == current
            || header.is_some_and(|h| stage.check_artifact_acceptable(stored, lref, h));
        match acceptable {
            true => stage.decode(record.data()).map(Some),
            false => Ok(None),
        }
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
    ) -> GExecResult<Self> {
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::errors::GExecError;
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
