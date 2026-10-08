//! The record of what's been processed and committed, and the rules for
//! changing it.
//!
//! This is everything the core keeps apart from the graph: the artifacts
//! the stages stand behind ([`StageArtifacts`]), where the pipeline stands
//! ([`CommitLog`]), which links have been loaded from the store, and the
//! store itself.  It never touches the provider, so it can't find links or
//! paths; it's told about them.
//!
//! The ledger is what jobs are issued from and what receipts are applied
//! to, and it's where every out-of-order ask is refused: a process job for
//! a stage whose deps don't cover the link, a commit job for a stage that's
//! already past the node, a receipt from somewhere the stage isn't recorded,
//! a cut to the committed path past a stage.  What it lets through keeps
//! the store consistent with what's in memory.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use strata_gchain_types::*;

use crate::artifact_cache::{ArtifactCache, ArtifactView};
use crate::errors::{GExecError, GExecResult};
use crate::store::{ArtifactRecord, ExecutorStore};
use crate::tracking::TrackingState;

use super::artifacts::StageArtifacts;
use super::commit_log::CommitLog;
use super::history::{ResumeSearch, StepJudge, find_resume_path};
use super::jobs::{CommitJob, InitJob, ProcessJob, PruneJob, Receipt, UncommitJob};
use super::pipeline::StagePipeline;
use super::stage::ProducedArtifact;

pub(crate) struct Ledger<S: GChainSpec, X: ExecutorStore<Spec = S>> {
    artifacts: StageArtifacts<S>,
    log: CommitLog<X>,

    /// Links whose stored artifacts have been loaded, and whether the store
    /// had any.
    loaded: HashMap<LinkRef<S>, bool>,

    store: Arc<X>,
}

impl<S: GChainSpec, X: ExecutorStore<Spec = S>> Ledger<S, X> {
    /// Opens the ledger a store holds, starting it off at `base_node` if the
    /// store has never been used.  Nothing is loaded yet.
    pub(super) fn open(
        pipeline: StagePipeline<S>,
        store: Arc<X>,
        base_node: NodeRef<S>,
    ) -> GExecResult<Self> {
        let log = CommitLog::open(store.clone(), base_node)?;
        Ok(Self {
            artifacts: StageArtifacts::new(pipeline),
            log,
            loaded: HashMap::new(),
            store,
        })
    }

    // ---- where things stand ------------------------------------------------

    pub(super) fn committed_node(&self) -> &NodeRef<S> {
        self.log.committed_node()
    }

    pub(super) fn committed_path(&self) -> &LinkPath<S> {
        self.log.committed_path()
    }

    pub(super) fn tracking(&self) -> &TrackingState<S> {
        self.log.tracking()
    }

    /// The stages' IDs in canonical order.
    pub(super) fn proc_ids(&self) -> impl DoubleEndedIterator<Item = ProcId> {
        self.artifacts.pipeline().proc_ids()
    }

    /// Every stage a stage depends on.  See
    /// [`StageSchedule::iter_deps`](super::schedule::StageSchedule::iter_deps).
    pub(super) fn iter_deps(&self, proc_id: ProcId) -> impl Iterator<Item = ProcId> {
        self.artifacts.pipeline().schedule().iter_deps(proc_id)
    }

    /// Asks a stage whether it can pick up processing from a node.
    pub(super) fn check_can_resume_at(
        &self,
        proc_id: ProcId,
        node: &NodeRef<S>,
    ) -> GExecResult<bool> {
        self.artifacts.get_stage(proc_id).check_can_resume_at(node)
    }

    /// Whether every stage stands at the committed node, which is what it
    /// takes to drive an executor over the ledger.
    pub(super) fn check_clean(&self) -> bool {
        self.proc_ids()
            .all(|id| self.tracking().get_stage_node(id) == Some(self.committed_node()))
    }

    /// Walks the commit history back from the committed node to find the
    /// links a stage has to redo and the node to redo them from.  See
    /// [`find_resume_path`].
    pub(super) fn find_resume_path(
        &self,
        floor: &NodeRef<S>,
        judge: &mut impl StepJudge<S>,
    ) -> GExecResult<ResumeSearch<S>> {
        find_resume_path(self.log.walker(), self.committed_node(), floor, judge)
    }

    // ---- artifacts ---------------------------------------------------------

    /// Whether a link's stored artifacts have been loaded, and if so whether
    /// the store had any.
    pub(super) fn check_loaded(&self, lref: &LinkRef<S>) -> Option<bool> {
        self.loaded.get(lref).copied()
    }

    /// Whether the store has anything at all for a link, without loading it.
    pub(super) fn check_present(&self, lref: &LinkRef<S>) -> GExecResult<bool> {
        match self.check_loaded(lref) {
            Some(present) => Ok(present),
            None => self
                .store
                .has_link_artifacts(lref)
                .map_err(GExecError::Storage),
        }
    }

    /// The artifact a stage has for a loaded link.
    pub(super) fn get_artifact<A: ProcArtifact>(
        &self,
        lref: &LinkRef<S>,
        proc_id: ProcId,
    ) -> Option<Arc<A>> {
        self.artifacts.get_artifact(lref, proc_id)
    }

    /// Whether a stage covers a loaded link.  See [`StageArtifacts::covers`].
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded.
    pub(super) fn covers(&self, lref: &LinkRef<S>, proc_id: ProcId) -> bool {
        self.assert_loaded(lref);
        self.artifacts.covers(lref, proc_id)
    }

    /// The stages that don't cover a loaded link, in canonical order.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded.
    pub(super) fn missing_stages(&self, lref: &LinkRef<S>) -> Vec<ProcId> {
        self.assert_loaded(lref);
        self.artifacts.missing_stages(lref)
    }

    /// Loads a link's stored artifacts if they aren't loaded already, after
    /// which the link counts as loaded whether or not the store had any.
    ///
    /// The link's header is only fetched from the provider if some artifact
    /// came from an older version of its stage, which the stage judges by it.
    pub(super) fn hydrate_link(
        &mut self,
        provider: &impl ChainProvider<Spec = S>,
        lref: &LinkRef<S>,
    ) -> GExecResult<()> {
        if self.check_loaded(lref).is_some() {
            return Ok(());
        }

        let records = self.load_stored(lref)?;
        let needs_header = records
            .iter()
            .any(|record| self.artifacts.check_needs_header(record));
        let header = match needs_header {
            true => provider.fetch_link_header(lref)?,
            false => None,
        };

        let present = !records.is_empty();
        for record in records {
            self.artifacts.insert_stored(record, header.as_ref())?;
        }
        self.loaded.insert(lref.clone(), present);
        Ok(())
    }

    /// Forgets a loaded link everywhere it's tracked, letting each stage
    /// clean up after its artifact before the artifacts go.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded, since the stages couldn't
    /// clean up after them.
    pub(super) fn forget_link(&mut self, lref: &LinkRef<S>) -> GExecResult<()> {
        self.assert_loaded(lref);
        self.artifacts.discard(lref)?;
        self.store
            .discard_link_artifacts(lref)
            .map_err(GExecError::Storage)?;
        self.loaded.remove(lref);
        Ok(())
    }

    /// Drops every loaded artifact except the committed path's.
    pub(super) fn evict_to_committed(&mut self) {
        let keep: HashSet<_> = self.committed_path().links().iter().cloned().collect();
        self.artifacts.retain_links(&keep);
        self.loaded.retain(|lref, _| keep.contains(lref));
    }

    // ---- jobs and receipts -------------------------------------------------

    /// Takes back what a job came to, bringing the record up to date with it.
    ///
    /// # Panics
    ///
    /// If the receipt doesn't follow from where the ledger has the stage,
    /// which means jobs were run out of turn.
    pub(super) fn apply_job_receipt(&mut self, receipt: impl Receipt<S>) -> GExecResult<()> {
        receipt.apply(self)
    }

    /// Initializing a stage at the committed node, if it has never been
    /// initialized.
    pub(super) fn create_init_job(&self, proc_id: ProcId) -> Option<InitJob<S>> {
        match self.tracking().get_stage_node(proc_id) {
            Some(_) => None,
            None => Some(InitJob::new(
                self.artifacts.get_stage(proc_id).clone(),
                self.committed_node().clone(),
            )),
        }
    }

    /// Running a stage on a loaded link whose pre-state is reached by a
    /// path.
    ///
    /// Refused unless every stage this one depends on covers what it has to:
    /// the link itself for a current-link dep, every link on the path for a
    /// path dep.  A stage that said the link is invalid covers nothing, so
    /// nothing that depends on it runs on the link.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded.
    pub(super) fn create_process_job<'r>(
        &'r self,
        lref: &'r LinkRef<S>,
        proc_id: ProcId,
        link: &'r Link<S>,
        path: &'r LinkPath<S>,
    ) -> GExecResult<ProcessJob<'r, S, ArtifactCache<S>>> {
        self.assert_loaded(lref);
        let coverage = self.artifacts.get_link_coverage(lref, path);
        self.artifacts
            .pipeline()
            .schedule()
            .check_ready(proc_id, &coverage)?;

        Ok(ProcessJob::new(
            self.artifacts.get_stage(proc_id),
            lref,
            link,
            path,
            self.artifacts.cache(),
        ))
    }

    /// The commits a stage has still to make to stand at a node on the
    /// committed path, in order, each with the artifacts it takes.
    ///
    /// The artifacts are looked up as the jobs are taken, which can fail
    /// where a stage has none for a link.
    ///
    /// # Panics
    ///
    /// If the stage isn't initialized or is already past the node.
    pub(super) fn create_commit_jobs(
        &self,
        proc_id: ProcId,
        upto: &NodeRef<S>,
    ) -> GExecResult<impl Iterator<Item = GExecResult<CommitJob<S>>> + '_> {
        let (stage_node, stage_pos) = self.get_stage_pos(proc_id);
        let pos = self.get_committed_index(upto)?;
        assert!(
            stage_pos <= pos,
            "gchain: committing a stage back past where it stands"
        );

        let stage = self.artifacts.get_stage(proc_id);
        let segments = self.log.find_segments_between(stage_node, upto)?;
        Ok(segments.into_iter().map(move |segment| {
            let artifacts = self.load_committed_with(proc_id, segment.links())?;
            Ok(CommitJob::new(stage.clone(), segment, artifacts))
        }))
    }

    /// The newest commit a stage has to undo to stand at a node on the
    /// committed path, if it isn't there yet, with the artifacts the stage
    /// committed it with.
    ///
    /// The stage is recorded at the start of the commit before the job is
    /// handed out, so this is asked again once the job has run.
    ///
    /// # Panics
    ///
    /// If the stage isn't initialized or is already before the node.
    pub(super) fn create_uncommit_job(
        &mut self,
        proc_id: ProcId,
        downto: &NodeRef<S>,
    ) -> GExecResult<Option<UncommitJob<S>>> {
        let (stage_node, stage_pos) = self.get_stage_pos(proc_id);
        let pos = self.get_committed_index(downto)?;
        assert!(
            stage_pos >= pos,
            "gchain: undoing a stage forward past where it stands"
        );

        let Some(segment) = self.log.find_segments_between(downto, stage_node)?.pop() else {
            return Ok(None);
        };

        let artifacts = self.load_committed_with(proc_id, segment.links())?;
        self.log
            .set_stage_node(proc_id, segment.base_node().clone())?;
        let stage = self.artifacts.get_stage(proc_id).clone();
        Ok(Some(UncommitJob::new(stage, segment, artifacts)))
    }

    /// Letting every stage discard what it kept for rolling back to before a
    /// node, in canonical order.
    pub(super) fn create_prune_jobs<'c>(
        &'c self,
        node: &'c NodeRef<S>,
    ) -> impl Iterator<Item = PruneJob<S>> + 'c {
        self.artifacts
            .pipeline()
            .iter_stages()
            .map(|stage| PruneJob::new(stage.clone(), node.clone()))
    }

    // ---- the committed path ------------------------------------------------

    /// Commits a path continuing from the committed node, which every stage
    /// has to cover every link of.  The stages then lag the committed node
    /// until each has committed the path.
    ///
    /// The artifacts of links off the committed path are dropped, since the
    /// path's own are the only ones a commit can't be undone without.
    ///
    /// # Panics
    ///
    /// If some stage isn't at the committed node to begin with.
    pub(super) fn append_commit(&mut self, path: &LinkPath<S>) -> GExecResult<()> {
        assert!(self.check_clean(), "gchain: commit over lagging stages");
        self.artifacts.check_committable(path)?;
        self.log.append(path)?;
        self.evict_to_committed();
        Ok(())
    }

    /// Checks that every stage could undo the committed links after a node,
    /// returning them.
    ///
    /// Only artifacts on hand count: a stage added after a link was
    /// committed has none for it, and nothing else is missing one once the
    /// stages stand behind everything committed.
    pub(super) fn check_undoable_to(&self, node: &NodeRef<S>) -> GExecResult<LinkPath<S>> {
        let undone = self.tracking().committed_path_from(node)?;
        self.artifacts.check_undoable(&undone)?;
        Ok(undone)
    }

    /// Cuts the committed path and the commit log back to a node on the
    /// path.
    ///
    /// # Panics
    ///
    /// If some stage is still past the node.
    pub(super) fn truncate_committed_to(&mut self, node: &NodeRef<S>) -> GExecResult<()> {
        let pos = self.get_committed_index(node)?;
        assert!(
            self.stage_indexes().all(|idx| idx <= pos),
            "gchain: committed path cut back past a stage"
        );
        self.log.truncate_to(node)
    }

    /// Moves the base of the committed path and the start of the commit log
    /// forward to a node on the path, giving up the links before it.  What
    /// hung off them is left for the core to sweep.
    ///
    /// # Panics
    ///
    /// If some stage is still before the node.
    pub(super) fn advance_committed_base_to(&mut self, node: &NodeRef<S>) -> GExecResult<()> {
        let pos = self.get_committed_index(node)?;
        assert!(
            self.stage_indexes().all(|idx| idx >= pos),
            "gchain: committed path's base moved past a stage"
        );
        self.log.advance_base_to(node)
    }

    // ---- what receipts do ---------------------------------------------------

    /// Records an artifact a stage produced for a loaded link, persisted
    /// before it's kept on hand.
    ///
    /// # Panics
    ///
    /// If the link's artifacts aren't loaded, since no process job is issued
    /// for such a link.
    pub(super) fn record_artifact(&mut self, produced: ProducedArtifact<S>) -> GExecResult<()> {
        let lref = produced.lref();
        assert!(
            self.check_loaded(lref).is_some(),
            "gchain: artifact for a link that isn't loaded"
        );
        self.store
            .store_artifact(produced.record())
            .map_err(GExecError::Storage)?;
        self.loaded.insert(lref.clone(), true);
        self.artifacts.insert_produced(produced);
        Ok(())
    }

    /// Records a stage as initialized at a node, which has to be the
    /// committed node.
    pub(super) fn record_stage_inited(
        &mut self,
        proc_id: ProcId,
        node: NodeRef<S>,
    ) -> GExecResult<()> {
        assert_eq!(
            &node,
            self.committed_node(),
            "gchain: stage initialized away from the committed node"
        );
        assert!(
            self.tracking().get_stage_node(proc_id).is_none(),
            "gchain: stage initialized twice"
        );
        self.log.set_stage_node(proc_id, node)
    }

    /// Records a stage as having made a commit, which has to continue from
    /// where it's recorded.
    pub(super) fn record_stage_committed(
        &mut self,
        proc_id: ProcId,
        from: &NodeRef<S>,
        to: NodeRef<S>,
    ) -> GExecResult<()> {
        assert_eq!(
            self.tracking().get_stage_node(proc_id),
            Some(from),
            "gchain: stage committed from somewhere it wasn't"
        );
        self.log.set_stage_node(proc_id, to)
    }

    /// Confirms a stage has undone a commit down to where it was recorded
    /// before the job went out.
    pub(super) fn record_stage_uncommitted(&self, proc_id: ProcId, node: &NodeRef<S>) {
        assert_eq!(
            self.tracking().get_stage_node(proc_id),
            Some(node),
            "gchain: stage undid a commit it wasn't recorded for"
        );
    }

    // ---- internals ---------------------------------------------------------

    /// Anything that reads or discards a link's artifacts has to have them
    /// loaded first, or it would silently act on none.
    fn assert_loaded(&self, lref: &LinkRef<S>) {
        assert!(
            self.check_loaded(lref).is_some(),
            "gchain: acting on a link that isn't loaded ({lref:?})"
        );
    }

    fn get_committed_index(&self, node: &NodeRef<S>) -> GExecResult<usize> {
        self.log
            .get_committed_index(node)
            .ok_or_else(|| GExecError::node_not_on_committed_path(node))
    }

    /// Where every initialized stage stands on the committed path.
    fn stage_indexes(&self) -> impl Iterator<Item = usize> {
        self.proc_ids()
            .filter_map(|proc_id| self.log.get_stage_index(proc_id))
    }

    /// The node a stage stands at and where that is on the committed path.
    ///
    /// # Panics
    ///
    /// If the stage isn't initialized.
    fn get_stage_pos(&self, proc_id: ProcId) -> (&NodeRef<S>, usize) {
        let stage_node = self
            .tracking()
            .get_stage_node(proc_id)
            .expect("gchain: moving a stage that isn't initialized");
        let stage_pos = self
            .log
            .get_stage_index(proc_id)
            .expect("gchain: moving a stage that isn't initialized");
        (stage_node, stage_pos)
    }

    /// Loads what the store has for a link.
    fn load_stored(&self, lref: &LinkRef<S>) -> GExecResult<Vec<ArtifactRecord<S>>> {
        self.store
            .load_link_artifacts(lref)
            .map_err(GExecError::Storage)
    }

    /// Collects the artifacts a stage committed a run of links with, in
    /// order: the ones it still stands behind, and otherwise what the store
    /// has from it, whichever version of the stage that came from.
    ///
    /// Reprocessing is the only time the store's differs from what's on
    /// hand, and it only ever rolls a stage back over a link before having
    /// it process the link again, so the store still has what the stage
    /// committed with.
    fn load_committed_with(
        &self,
        proc_id: ProcId,
        links: &[LinkRef<S>],
    ) -> GExecResult<Vec<Arc<dyn DynProcArtifact>>> {
        let mut artifacts = Vec::with_capacity(links.len());
        for lref in links {
            if let Some(artifact) = self.artifacts.cache().get_artifact_dyn(lref, proc_id) {
                artifacts.push(artifact.clone());
                continue;
            }

            let record = self
                .load_stored(lref)?
                .into_iter()
                .find(|record| record.proc_id() == proc_id)
                .ok_or_else(|| GExecError::stale_artifact(lref, proc_id))?;
            artifacts.push(self.artifacts.get_stage(proc_id).decode(record.data())?);
        }

        Ok(artifacts)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::core::StageJob;
    use crate::core::pipeline::PipelineBuilder;
    use crate::mem_store::MemExecutorStore;
    use crate::test_support::*;

    fn id(s: &str) -> ProcId {
        ProcId::from_str(s).expect("test: parse ProcId")
    }

    fn ids(names: &[&str]) -> Vec<ProcId> {
        names.iter().map(|n| id(n)).collect()
    }

    fn ledger(pipeline: StagePipeline<TestSpec>) -> Ledger<TestSpec, MemExecutorStore<TestSpec>> {
        let store = Arc::new(MemExecutorStore::new());
        let mut ledger = Ledger::open(pipeline, store, TestRef(1)).expect("test: open ledger");
        ledger
            .hydrate_link(&TestProvider::new(), &TestRef(10))
            .expect("test: load link");
        ledger
    }

    /// Runs one stage on link 10 and records what it made.
    fn process(
        ledger: &mut Ledger<TestSpec, MemExecutorStore<TestSpec>>,
        proc: &str,
    ) -> GExecResult<()> {
        let (lref, link) = (TestRef(10), TestLink(10));
        let path = LinkPath::new_at(TestRef(1));
        let job = ledger.create_process_job(&lref, id(proc), &link, &path)?;
        ledger.apply_job_receipt(job.run()?)
    }

    /// A stage runs only once the stages it depends on cover the link, and
    /// what each stage produces is on record as soon as it's applied.
    #[test]
    fn test_process_jobs_wait_for_their_deps_to_cover_the_link() {
        let cur_dep = ProcDeps::new(ids(&["a"]), Vec::new());
        let pipeline = PipelineBuilder::new()
            .add_stage(
                id("a"),
                TestProc::new(),
                ProcDeps::new(Vec::new(), Vec::new()),
            )
            .expect("test: add stage")
            .add_stage(id("b"), TestProc::new(), cur_dep)
            .expect("test: add stage")
            .build();
        let mut ledger = ledger(pipeline);

        let err = expect_err(process(&mut ledger, "b"), "stage b to be missing its dep");
        assert!(matches!(err, GExecError::UnmetDep { .. }));
        assert_eq!(ledger.missing_stages(&TestRef(10)), ids(&["a", "b"]));

        process(&mut ledger, "a").expect("test: run a");
        assert_eq!(ledger.missing_stages(&TestRef(10)), ids(&["b"]));
        process(&mut ledger, "b").expect("test: run b");
        assert!(ledger.missing_stages(&TestRef(10)).is_empty());
        assert!(ledger.check_present(&TestRef(10)).expect("test: present"));
    }

    /// A stage's finding that a link is invalid is kept, but covers nothing,
    /// so a stage that depends on it can't run on the link.
    #[test]
    fn test_invalid_artifact_is_recorded_but_covers_nothing() {
        let cur_dep = ProcDeps::new(ids(&["a"]), Vec::new());
        let pipeline = PipelineBuilder::new()
            .add_stage(
                id("a"),
                TestProc::new().rejecting([10]),
                ProcDeps::new(Vec::new(), Vec::new()),
            )
            .expect("test: add stage")
            .add_stage(id("b"), TestProc::new(), cur_dep)
            .expect("test: add stage")
            .build();
        let mut ledger = ledger(pipeline);

        process(&mut ledger, "a").expect("test: run a");

        assert!(
            ledger
                .get_artifact::<FlagArtifact>(&TestRef(10), id("a"))
                .is_some()
        );
        assert!(!ledger.covers(&TestRef(10), id("a")));
        assert_eq!(ledger.missing_stages(&TestRef(10)), ids(&["a", "b"]));
        let err = expect_err(process(&mut ledger, "b"), "stage b to be blocked by a");
        assert!(matches!(err, GExecError::UnmetDep { .. }));
    }
}
