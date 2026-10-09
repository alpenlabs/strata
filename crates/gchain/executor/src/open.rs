//! High-level logic related to opening an executor on top of a store.
//!
//! Opening goes through up to three steps: the store is opened and the
//! committed path's artifacts loaded, committed links the stages no longer
//! stand behind are reprocessed, and the stages are brought level with the
//! committed node.  Only then is there an executor to drive.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::core::{ExecutorCore, StagePipeline};
use crate::errors::{GExecError, GExecResult};
use crate::reprocess::{RejectedCommit, ReprocLog, StagePlan, apply_next_plan, plan_reprocessing};
use crate::serial_executor::SerialExecutor;
use crate::serial_ops::{level_stage, run_job_and_apply_receipt};
use crate::store::ExecutorStore;

/// What opening an executor did to bring the stored state in line with the
/// pipeline it was opened with.
pub struct OpenReport<S: GChainSpec> {
    initialized: Vec<ProcId>,
    recommitted: Vec<ProcId>,
    reproc: ReprocLog<S>,
}

impl<S: GChainSpec> OpenReport<S> {
    /// Stages that had no committed state and were initialized at the
    /// committed node.
    pub fn initialized(&self) -> &[ProcId] {
        &self.initialized
    }

    /// Stages whose committed node lagged the pipeline's and had the rest of
    /// the committed path committed again.
    pub fn recommitted(&self) -> &[ProcId] {
        &self.recommitted
    }

    /// The committed links each stage processed again, because it no longer
    /// accepted the artifacts stored for them.
    pub fn reprocessed(&self) -> &[StagePlan<S>] {
        self.reproc.applied()
    }

    /// Committed links a stage rejected when processing them again, which
    /// the pipeline was rolled back to before.
    pub fn rejected(&self) -> &[RejectedCommit<S>] {
        self.reproc.rejected()
    }
}

/// A [`SerialExecutor`] and the output produced from readying it.
pub(crate) type SerialExecutorReadyOutput<S, P, X> = (SerialExecutor<S, P, X>, OpenReport<S>);

/// What opening an executor over a store came to.
pub enum OpenResult<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>> {
    /// The stages stand behind everything that's committed.
    Ready(SerialExecutor<S, P, X>, OpenReport<S>),

    /// Some stages have to reprocessed because some links due to a client
    /// upgrade or crash.
    NeedsReprocess(ReprocExecutor<S, P, X>),
}

impl<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>> OpenResult<S, P, X> {
    /// Does whatever reprocessing is needed right away and hands over the
    /// executor.
    pub fn finish(self) -> GExecResult<SerialExecutorReadyOutput<S, P, X>> {
        match self {
            Self::Ready(exec, report) => Ok((exec, report)),
            Self::NeedsReprocess(pending) => pending.finish(),
        }
    }

    /// Unwraps the `Ready` case if present.  This is a convenience fn meant for
    /// tests.
    ///
    /// # Panics
    ///
    /// If not ready, meaning there's reprocessing needed.
    pub fn expect_ready(self) -> SerialExecutorReadyOutput<S, P, X> {
        match self {
            OpenResult::Ready(exec, report) => (exec, report),
            OpenResult::NeedsReprocess(_) => panic!("gchain/open: not ready"),
        }
    }
}

/// Executor over a store with committed links that have to be reprocessed
/// before it can be driven.
///
/// The plans are applied a stage at a time in canonical order, since a stage
/// builds on what the ones it depends on produce.  When to do that is up to
/// the caller; [`Self::finish`] applies whatever is left and hands over the
/// executor.
pub struct ReprocExecutor<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>> {
    core: ExecutorCore<S, P, X>,
    log: ReprocLog<S>,
}

impl<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>
    ReprocExecutor<S, P, X>
{
    /// The node the pipeline is committed up to.
    pub fn committed_node(&self) -> &NodeRef<S> {
        self.core.committed_node()
    }

    /// The committed links that can still be rolled back, from the oldest.
    pub fn committed_path(&self) -> &LinkPath<S> {
        self.core.committed_path()
    }

    /// What has been reprocessed so far and what's left.
    pub fn log(&self) -> &ReprocLog<S> {
        &self.log
    }

    /// Applies the next stage's plan, returning the stage if there was one
    /// left.
    ///
    /// If the stage rejects a link, the whole pipeline is rolled back to the
    /// link's origin and the plans left are cut short to match.
    pub fn apply_next_stage(&mut self) -> GExecResult<Option<ProcId>> {
        apply_next_plan(&mut self.core, &mut self.log)
    }

    /// Applies the plans that are left and hands over the executor.
    pub fn finish(mut self) -> GExecResult<SerialExecutorReadyOutput<S, P, X>> {
        while self.apply_next_stage()?.is_some() {
            // TODO(trey): add logging about what we're doing here
        }
        finish_open(self.core, self.log)
    }
}

/// Opens an executor over a store, reconciling what it holds with the
/// pipeline's stages as far as that goes without reprocessing anything.
pub(crate) fn open_executor<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
>(
    pipeline: StagePipeline<S>,
    provider: Arc<P>,
    store: Arc<X>,
    base_node: NodeRef<S>,
) -> GExecResult<OpenResult<S, P, X>> {
    let core = ExecutorCore::open(pipeline, provider, store, base_node)?;
    let plans = plan_reprocessing(&core)?;
    if !plans.is_empty() {
        let log = ReprocLog::new(plans);
        return Ok(OpenResult::NeedsReprocess(ReprocExecutor { core, log }));
    }

    let (exec, report) = finish_open(core, ReprocLog::new(Vec::new()))?;
    Ok(OpenResult::Ready(exec, report))
}

/// Brings the stages level with the committed node, which is all that's left
/// to do once no committed link needs reprocessing, and hands over the
/// executor with the account of what opening it took.
fn finish_open<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>(
    mut core: ExecutorCore<S, P, X>,
    reproc: ReprocLog<S>,
) -> GExecResult<SerialExecutorReadyOutput<S, P, X>> {
    let leveled = level_stages(&mut core)?;
    let report = OpenReport {
        initialized: leveled.initialized,
        recommitted: leveled.recommitted,
        reproc,
    };
    Ok((SerialExecutor::from_clean_core(core), report))
}

/// The stages that had to be brought level with the committed node.
struct LeveledStages {
    initialized: Vec<ProcId>,
    recommitted: Vec<ProcId>,
}

/// Brings every stage's committed state level with the committed node: a
/// stage that has never been initialized is initialized there, and one that
/// lags has the rest of the committed path committed.
fn level_stages<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>(
    core: &mut ExecutorCore<S, P, X>,
) -> GExecResult<LeveledStages> {
    let mut leveled = LeveledStages {
        initialized: Vec::new(),
        recommitted: Vec::new(),
    };

    let committed = core.committed_node().clone();
    let proc_ids: Vec<_> = core.proc_ids().collect();
    for proc_id in proc_ids {
        match core.get_stage_node(proc_id).cloned() {
            None => {
                let job = core
                    .create_init_job(proc_id)
                    .expect("gchain: stage without a node has no init job");
                run_job_and_apply_receipt(core, job)?;
                leveled.initialized.push(proc_id);
            }
            Some(node) if node == committed => {}
            Some(node) => {
                if core.committed_path().get_node_index(&node).is_none() {
                    return Err(GExecError::stage_diverged(proc_id, &node));
                }
                level_stage(core, proc_id, &committed)?;
                leveled.recommitted.push(proc_id);
            }
        }
    }

    Ok(leveled)
}
