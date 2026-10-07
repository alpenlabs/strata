//! The bookkeeping an executor is built on, kept in step with the store.
//!
//! Three things are kept track of here: where the pipeline stands (the
//! [`CommitLog`]), which links have artifacts and where those are (the store,
//! with the ones in use loaded into the [`StageArtifacts`]), and how the links
//! hang together (asked of the provider whenever it matters).  Every step that
//! changes any of it leaves the store consistent with what's in memory.
//!
//! What isn't here is stage work.  The core never has a stage process,
//! commit, or undo anything, and never decides which stage should: it hands
//! out [`jobs`] with everything a call into a stage takes, and whoever holds
//! the core runs them where and when it likes and hands back the receipts for
//! the core to [`apply`](ExecutorCore::apply_job_receipt).  So it promises
//! neither that the stages sit at the same node nor that committed links have
//! artifacts, and executors that go about the work differently can be built on
//! the same core.
//!
//! What it does promise is that it can't be told something that isn't so.
//! A receipt can only come from running a job, and the core checks each one
//! against where it has the stage.  A stage is recorded after moving forward
//! and before moving back (see [`CommitLog::set_stage_node`]), so a job
//! that's dropped or fails only ever leaves a stage recorded behind where it
//! is, which is caught up by committing the difference again.  Every cut to
//! the committed path checks where the stages stand first.

mod artifacts;
mod commit_log;
mod executor;
mod history;
mod jobs;
mod ledger;
mod pipeline;
mod process;
mod schedule;
mod stage;
mod traverse;

pub(crate) use executor::ExecutorCore;
pub(crate) use history::{CommittedStep, ResumeSearch, StepJudge};
pub(crate) use jobs::{Receipt, StageJob};
pub(crate) use pipeline::{PipelineBuilder, StagePipeline};

/// What only the core's own receipts may implement.
mod sealed {
    pub(crate) trait Sealed {}
}
