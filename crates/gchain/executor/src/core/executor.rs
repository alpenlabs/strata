//! The core itself: the [`Ledger`] together with the provider, which is
//! what it takes to find links and paths and to load what the store has for
//! them.  See the [module docs](super) for what it promises.

use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::ArtifactCache;
use crate::errors::{GExecError, GExecResult};
use crate::store::ExecutorStore;

use super::history::{ResumeSearch, StepJudge};
use super::jobs::{CommitJob, InitJob, ProcessJob, PruneJob, Receipt, UncommitJob};
use super::ledger::Ledger;
use super::pipeline::StagePipeline;
use super::traverse::{find_path, find_reachable_links};

/// Lower-level internal executor that contains all of the data involved in
/// running an executor but does not make high-level decisions about managing
/// it.
pub(crate) struct ExecutorCore<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
> {
    ledger: Ledger<S, X>,
    provider: Arc<P>,
}

impl<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>> ExecutorCore<S, P, X> {
    /// Opens a store, starting it off at `base_node` if it has never been
    /// used, and loads the committed path's artifacts from it.
    pub(crate) fn open(
        pipeline: StagePipeline<S>,
        provider: Arc<P>,
        store: Arc<X>,
        base_node: NodeRef<S>,
    ) -> GExecResult<Self> {
        let ledger = Ledger::open(pipeline, store, base_node)?;
        let mut core = Self { ledger, provider };
        let committed: Vec<_> = core.committed_path().links().to_vec();
        for lref in &committed {
            core.hydrate_link(lref)?;
        }
        Ok(core)
    }

    // ---- where things stand ------------------------------------------------

    pub(crate) fn committed_node(&self) -> &NodeRef<S> {
        self.ledger.committed_node()
    }

    pub(crate) fn committed_path(&self) -> &LinkPath<S> {
        self.ledger.committed_path()
    }

    /// Whether a link is on the committed path.
    pub(crate) fn is_committed(&self, lref: &LinkRef<S>) -> bool {
        self.ledger.is_committed(lref)
    }

    /// See [`Ledger::get_stage_node`].
    pub(crate) fn get_stage_node(&self, proc_id: ProcId) -> Option<&NodeRef<S>> {
        self.ledger.get_stage_node(proc_id)
    }

    /// See [`Ledger::get_stage_floor`].
    pub(crate) fn get_stage_floor(&self, proc_id: ProcId) -> Option<&NodeRef<S>> {
        self.ledger.get_stage_floor(proc_id)
    }

    /// The stages' IDs in canonical order.
    pub(crate) fn proc_ids(&self) -> impl DoubleEndedIterator<Item = ProcId> {
        self.ledger.proc_ids()
    }

    /// Every stage a stage depends on.
    pub(crate) fn iter_proc_deps(&self, proc_id: ProcId) -> impl Iterator<Item = ProcId> {
        self.ledger.iter_deps(proc_id)
    }

    /// Asks a stage whether it can pick up processing from a node.
    pub(crate) fn check_can_resume_at(
        &self,
        proc_id: ProcId,
        node: &NodeRef<S>,
    ) -> GExecResult<bool> {
        self.ledger.check_can_resume_at(proc_id, node)
    }

    /// Whether every stage stands at the committed node.  See
    /// [`Ledger::check_clean`].
    pub(crate) fn check_clean(&self) -> bool {
        self.ledger.check_clean()
    }

    #[cfg(test)]
    pub(crate) fn check_loaded(&self, lref: &LinkRef<S>) -> bool {
        self.ledger.check_loaded(lref)
    }

    /// See [`Ledger::find_resume_path`].
    pub(crate) fn find_resume_path(
        &self,
        floor: &NodeRef<S>,
        judge: &mut impl StepJudge<S>,
    ) -> GExecResult<ResumeSearch<S>> {
        self.ledger.find_resume_path(floor, judge)
    }

    // ---- artifacts ---------------------------------------------------------

    /// The artifact a stage has for a link, loading the link's artifacts if
    /// they aren't loaded yet.
    pub(crate) fn get_artifact<A: ProcArtifact>(
        &mut self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> GExecResult<Option<Arc<A>>> {
        self.hydrate_link(lref)?;
        Ok(self.ledger.get_artifact(lref, proc_id))
    }

    /// Whether a stage covers a loaded link.  See [`Ledger::covers`].
    pub(crate) fn covers(&self, lref: &LinkRef<S>, proc_id: ProcId) -> bool {
        self.ledger.covers(lref, proc_id)
    }

    /// The stages that don't cover a link, loading the link's artifacts if
    /// they aren't loaded yet.
    pub(crate) fn get_missing_stages(&mut self, lref: &LinkRef<S>) -> GExecResult<Vec<ProcId>> {
        self.hydrate_link(lref)?;
        Ok(self.ledger.missing_stages(lref))
    }

    /// Whether every stage covers a link, which is what lets a path run
    /// through it.  Loads the link's artifacts on the way if they aren't
    /// loaded yet.
    pub(crate) fn check_usable(&mut self, lref: &LinkRef<S>) -> GExecResult<bool> {
        self.hydrate_link(lref)?;
        Ok(self.ledger.missing_stages(lref).is_empty())
    }

    /// Whether the store has anything at all for a link, without loading it.
    pub(crate) fn check_present(&self, lref: &LinkRef<S>) -> GExecResult<bool> {
        self.ledger.check_present(lref)
    }

    /// Forgets every link that's reachable from a node but no longer from the
    /// committed path's base, returning them.
    pub(crate) fn sweep_from(&mut self, node: &NodeRef<S>) -> GExecResult<Vec<LinkRef<S>>> {
        let provider = self.provider.clone();
        let base = self.committed_path().base_node().clone();
        let keep = find_reachable_links(provider.as_ref(), &base, |l| self.check_present(l))?;
        let candidates = find_reachable_links(provider.as_ref(), node, |l| self.check_present(l))?;

        let doomed: Vec<_> = candidates
            .into_iter()
            .filter(|l| !keep.contains(l))
            .collect();
        for lref in &doomed {
            self.forget_link(lref)?;
        }

        Ok(doomed)
    }

    /// Forgets a link everywhere it's tracked, letting each stage clean up
    /// after its artifact before the artifacts go.
    pub(crate) fn forget_link(&mut self, lref: &LinkRef<S>) -> GExecResult<()> {
        self.hydrate_link(lref)?;
        self.ledger.forget_link(lref)
    }

    /// Drops every loaded artifact except the committed path's.
    pub(crate) fn evict_to_committed(&mut self) {
        self.ledger.evict_to_committed();
    }

    // ---- the graph ---------------------------------------------------------

    /// Fetches a link from the underlying provider and repackages the errors to
    /// gobble missing links.
    pub(crate) fn fetch_link(&self, lref: &LinkRef<S>) -> GExecResult<Link<S>> {
        self.provider
            .fetch_link(lref)?
            .ok_or_else(|| GExecError::missing_link(lref))
    }

    /// Fetches the nodes a link connects, which is how the executor knows where
    /// the link sits relative to what it has processed.
    pub(crate) fn fetch_link_endpoints(&self, lref: &LinkRef<S>) -> GExecResult<LinkEndpoints<S>> {
        self.provider
            .fetch_link_endpoints(lref)?
            .ok_or_else(|| GExecError::missing_link_endpoints(lref))
    }

    /// A path of usable links from the committed node to a link's origin,
    /// with the fewest links.
    pub(crate) fn path_to_origin(
        &mut self,
        lref: &LinkRef<S>,
        origin: &NodeRef<S>,
    ) -> GExecResult<LinkPath<S>> {
        let provider = self.provider.clone();
        let committed = self.committed_node().clone();
        find_path(provider.as_ref(), &committed, origin, |l| {
            self.check_usable(l)
        })?
        .ok_or_else(|| GExecError::origin_unreachable(lref))
    }

    // ---- jobs and receipts -------------------------------------------------

    /// See [`Ledger::apply_job_receipt`].
    pub(crate) fn apply_job_receipt(&mut self, receipt: impl Receipt<S>) -> GExecResult<()> {
        self.ledger.apply_job_receipt(receipt)
    }

    /// See [`Ledger::create_init_job`].
    pub(crate) fn create_init_job(&self, proc_id: ProcId) -> Option<InitJob<S>> {
        self.ledger.create_init_job(proc_id)
    }

    /// Running a stage on a link whose pre-state is reached by a path,
    /// loading the link's artifacts first if they aren't loaded yet.  See
    /// [`Ledger::create_process_job`].
    pub(crate) fn create_process_job<'r>(
        &'r mut self,
        lref: &'r LinkRef<S>,
        proc_id: ProcId,
        link: &'r Link<S>,
        path: &'r LinkPath<S>,
    ) -> GExecResult<ProcessJob<'r, S, ArtifactCache<S>>> {
        self.hydrate_link(lref)?;
        self.ledger.create_process_job(lref, proc_id, link, path)
    }

    /// See [`Ledger::create_commit_jobs`].
    pub(crate) fn create_commit_jobs(
        &self,
        proc_id: ProcId,
        upto: &NodeRef<S>,
    ) -> GExecResult<impl Iterator<Item = GExecResult<CommitJob<S>>> + '_> {
        self.ledger.create_commit_jobs(proc_id, upto)
    }

    /// See [`Ledger::create_uncommit_job`].
    pub(crate) fn create_uncommit_job(
        &mut self,
        proc_id: ProcId,
        downto: &NodeRef<S>,
    ) -> GExecResult<Option<UncommitJob<S>>> {
        self.ledger.create_uncommit_job(proc_id, downto)
    }

    /// See [`Ledger::create_prune_jobs`].
    pub(crate) fn create_prune_jobs<'c>(
        &'c self,
        node: &'c NodeRef<S>,
    ) -> impl Iterator<Item = PruneJob<S>> + 'c {
        self.ledger.create_prune_jobs(node)
    }

    // ---- the committed path ------------------------------------------------

    /// See [`Ledger::append_commit`].
    pub(crate) fn append_commit(&mut self, path: &LinkPath<S>) -> GExecResult<()> {
        self.ledger.append_commit(path)
    }

    /// See [`Ledger::check_undoable_to`].
    pub(crate) fn check_undoable_to(&self, node: &NodeRef<S>) -> GExecResult<LinkPath<S>> {
        self.ledger.check_undoable_to(node)
    }

    /// See [`Ledger::truncate_committed_to`].
    pub(crate) fn truncate_committed_to(&mut self, node: &NodeRef<S>) -> GExecResult<()> {
        self.ledger.truncate_committed_to(node)
    }

    /// Moves the base of the committed path forward to a node on the path,
    /// forgetting every link that can't be reached from it any more and
    /// returning them.  See [`Ledger::advance_committed_base_to`].
    pub(crate) fn advance_committed_base_to(
        &mut self,
        node: &NodeRef<S>,
    ) -> GExecResult<Vec<LinkRef<S>>> {
        // The base moves first: if discarding is cut short, what's left
        // behind is unreachable from the new base and a later sweep finds it.
        let old_base = self.committed_path().base_node().clone();
        self.ledger.advance_committed_base_to(node)?;
        self.sweep_from(&old_base)
    }

    // ---- internals ---------------------------------------------------------

    /// See [`Ledger::hydrate_link`].
    fn hydrate_link(&mut self, lref: &LinkRef<S>) -> GExecResult<()> {
        self.ledger.hydrate_link(self.provider.as_ref(), lref)
    }
}
