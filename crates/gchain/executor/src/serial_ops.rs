//! Stage work done a job at a time on the calling thread.
//!
//! These are the loops the serial executor, opening, and reprocessing share,
//! since the latter two run before there's an executor to drive.  Each takes
//! jobs from the core, runs them, and hands the receipts back.  An executor
//! that spreads the work out would run the same jobs from wherever it likes
//! and record the receipts the same way.
//!
//! The one piece of strategy here is [`run_link`]: it runs a link's stages
//! in canonical order and stops at the first that finds the link invalid.
//! The core doesn't mind a stage running on a link another stage found
//! invalid, as long as it doesn't depend on that stage; stopping is the
//! serial executor's choice.

use strata_gchain_types::*;

use crate::core::{ExecutorCore, Receipt, StageJob};
use crate::errors::GExecResult;
use crate::store::ExecutorStore;

/// What a serial run of a link's stages came to.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LinkOutcome {
    /// Every stage found the link valid, so every stage covers it and it
    /// can be built on.
    Accepted,

    /// A stage found the link invalid.  No stage after it was run on the
    /// link.
    Rejected { proc_id: ProcId },
}

impl LinkOutcome {
    pub fn is_accepted(&self) -> bool {
        matches!(self, LinkOutcome::Accepted)
    }

    pub fn is_rejected(&self) -> bool {
        matches!(self, LinkOutcome::Rejected { .. })
    }
}

/// Runs a job and records what it came to.
pub(crate) fn run_job_and_apply_receipt<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
>(
    core: &mut ExecutorCore<S, P, X>,
    job: impl StageJob<S, Receipt: Receipt<S>>,
) -> GExecResult<()> {
    core.apply_job_receipt(job.run()?)
}

/// Runs some stages on a link whose pre-state is reached by a path, in the
/// order given, recording what each produced as it goes and stopping at the
/// first that finds the link invalid.
///
/// What a rejected link leaves on record is for the caller to deal with.
pub(crate) fn run_link<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>(
    core: &mut ExecutorCore<S, P, X>,
    lref: &LinkRef<S>,
    path: &LinkPath<S>,
    stages: &[ProcId],
) -> GExecResult<LinkOutcome> {
    let link = core.fetch_link(lref)?;
    for proc_id in stages {
        let receipt = core
            .create_process_job(lref, *proc_id, &link, path)?
            .run()?;
        let valid = receipt.is_link_valid();
        core.apply_job_receipt(receipt)?;
        if !valid {
            return Ok(LinkOutcome::Rejected { proc_id: *proc_id });
        }
    }
    Ok(LinkOutcome::Accepted)
}

/// Commits the committed links between where a stage stands and a later
/// node in the stage, a commit at a time as they were first committed,
/// leaving the stage at the node.
pub(crate) fn level_stage<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>(
    core: &mut ExecutorCore<S, P, X>,
    proc_id: ProcId,
    upto: &NodeRef<S>,
) -> GExecResult<()> {
    let jobs: Vec<_> = core
        .create_commit_jobs(proc_id, upto)?
        .collect::<Result<_, _>>()?;
    for job in jobs {
        run_job_and_apply_receipt(core, job)?;
    }
    Ok(())
}

/// Undoes the committed links between where a stage stands and an earlier
/// node in the stage, a commit at a time from the newest, leaving the stage
/// at the node.
pub(crate) fn rewind_stage<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
>(
    core: &mut ExecutorCore<S, P, X>,
    proc_id: ProcId,
    downto: &NodeRef<S>,
) -> GExecResult<()> {
    while let Some(job) = core.create_uncommit_job(proc_id, downto)? {
        run_job_and_apply_receipt(core, job)?;
    }
    Ok(())
}
