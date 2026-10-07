//! Describing an executor to be opened: its stages, and the provider and
//! store it runs over.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::core::PipelineBuilder;
use crate::errors::GExecResult;
use crate::open::{OpenResult, open_executor};
use crate::store::ExecutorStore;

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
    ) -> GExecResult<Self> {
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
    pub fn open(self) -> GExecResult<OpenResult<S, P, X>> {
        open_executor(
            self.pipeline.build(),
            self.provider,
            self.store,
            self.base_node,
        )
    }
}
