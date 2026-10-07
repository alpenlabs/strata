//! Reprocessing committed links a stage no longer stands behind.
//!
//! Opening a store with newer stages than it was written by can find
//! committed links whose artifacts the stages don't accept any more.  Each
//! such stage gets a plan: the committed links it has to process again, from
//! the node it can resume at up to the committed node.  Applying a plan rolls
//! just that stage back with the artifacts it committed with, runs it over
//! the links again, and commits what that produced.
//!
//! This is the only time a stage is handed artifacts it doesn't stand
//! behind any more, and only to undo what they committed.
//!
//! Until every plan is applied there's no executor to drive, only a
//! [`crate::ReprocExecutor`] that knows what's left to do.  This module
//! works out the plans and applies them; that executor is what hands them
//! out.

use std::collections::VecDeque;
use std::mem;

use strata_gchain_types::*;

use crate::core::{CommittedStep, ExecutorCore, ResumeSearch, StepJudge};
use crate::errors::{GExecError, GExecResult};
use crate::serial_ops::{level_stage, rewind_stage, run_link};
use crate::store::ExecutorStore;

/// The committed links a stage has to process again.
pub struct StagePlan<S: GChainSpec> {
    proc_id: ProcId,
    path: LinkPath<S>,
}

impl<S: GChainSpec> StagePlan<S> {
    pub fn proc_id(&self) -> ProcId {
        self.proc_id
    }

    /// The links to process again, as the path from the node the stage
    /// resumes at.
    pub fn path(&self) -> &LinkPath<S> {
        &self.path
    }
}

/// A committed link that a stage rejected when processing it again.
///
/// The pipeline was rolled back to the link's origin, and the link forgotten
/// along with everything only reachable through it.
pub struct RejectedCommit<S: GChainSpec> {
    lref: LinkRef<S>,
    proc_id: ProcId,
    dropped: Vec<LinkRef<S>>,
}

impl<S: GChainSpec> RejectedCommit<S> {
    pub fn lref(&self) -> &LinkRef<S> {
        &self.lref
    }

    /// The stage that rejected the link.
    pub fn proc_id(&self) -> ProcId {
        self.proc_id
    }

    /// Every link forgotten, the rejected one included.
    pub fn dropped(&self) -> &[LinkRef<S>] {
        &self.dropped
    }
}

/// What reprocessing has done so far and what it has left to do.
pub struct ReprocLog<S: GChainSpec> {
    pending: VecDeque<StagePlan<S>>,
    applied: Vec<StagePlan<S>>,
    rejected: Vec<RejectedCommit<S>>,
}

impl<S: GChainSpec> ReprocLog<S> {
    pub(crate) fn new(plans: Vec<StagePlan<S>>) -> Self {
        Self {
            pending: plans.into(),
            applied: Vec::new(),
            rejected: Vec::new(),
        }
    }

    /// The plans still to be applied, in the order they will be.
    pub fn pending(&self) -> impl Iterator<Item = &StagePlan<S>> {
        self.pending.iter()
    }

    /// The links each stage processed again, in the order the stages did.
    pub fn applied(&self) -> &[StagePlan<S>] {
        &self.applied
    }

    /// Committed links that were rejected when processed again, which the
    /// pipeline was rolled back to before.
    pub fn rejected(&self) -> &[RejectedCommit<S>] {
        &self.rejected
    }

    fn take_next(&mut self) -> Option<StagePlan<S>> {
        self.pending.pop_front()
    }

    fn record_applied(&mut self, plan: StagePlan<S>) {
        self.applied.push(plan);
    }

    fn record_rejected(&mut self, rejected: RejectedCommit<S>) {
        self.rejected.push(rejected);
    }

    /// Cuts the plans that are left down to the links still committed after
    /// a rollback.
    fn cut_pending_to(&mut self, committed: &LinkPath<S>) {
        let pending = mem::take(&mut self.pending);
        self.pending = pending
            .into_iter()
            .filter_map(|plan| cut_plan_to(plan, committed))
            .collect();
    }
}

/// Cuts a plan down to the part of it that's on the committed path, if any
/// of it is.
fn cut_plan_to<S: GChainSpec>(plan: StagePlan<S>, committed: &LinkPath<S>) -> Option<StagePlan<S>> {
    let resume_pos = committed.get_node_index(plan.path.base_node())?;
    let path = committed.slice(resume_pos, committed.len());
    (!path.is_empty()).then_some(StagePlan {
        proc_id: plan.proc_id,
        path,
    })
}

/// Applies the next plan in a log, returning its stage if there was one
/// left.
///
/// If the stage rejects a link, the whole pipeline is rolled back to the
/// link's origin and the plans left are cut short to match.
pub(crate) fn apply_next_plan<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
>(
    core: &mut ExecutorCore<S, P, X>,
    log: &mut ReprocLog<S>,
) -> GExecResult<Option<ProcId>> {
    let Some(plan) = log.take_next() else {
        return Ok(None);
    };
    let proc_id = plan.proc_id;

    match reprocess_stage(core, &plan)? {
        None => log.record_applied(plan),
        Some(rejection) => {
            let dropped = roll_back_rejected(core, proc_id, &rejection)?;
            log.record_applied(StagePlan {
                proc_id,
                path: plan.path.slice(0, rejection.plan_pos),
            });
            log.record_rejected(RejectedCommit {
                lref: rejection.lref,
                proc_id,
                dropped,
            });
            log.cut_pending_to(core.committed_path());
        }
    }

    Ok(Some(proc_id))
}

/// Judges committed links for one stage by the artifacts loaded for them and
/// by what the stages it depends on are reprocessing.
struct StageJudge<'c, S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>> {
    core: &'c ExecutorCore<S, P, X>,
    proc_id: ProcId,

    /// The plans of the stages before this one.
    plans: &'c [StagePlan<S>],
}

impl<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>
    StageJudge<'_, S, P, X>
{
    /// Whether a stage this one depends on is reprocessing a link, which
    /// makes what this one built on its artifact out of date.
    fn check_dep_reprocesses(&self, lref: &LinkRef<S>) -> bool {
        self.core
            .iter_proc_deps(self.proc_id)
            .any(|dep| check_planned(self.plans, dep, lref))
    }
}

impl<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>> StepJudge<S>
    for StageJudge<'_, S, P, X>
{
    fn check_step_acceptable(&mut self, step: &CommittedStep<S>) -> GExecResult<bool> {
        let lref = step.lref();
        Ok(self.core.covers(lref, self.proc_id) && !self.check_dep_reprocesses(lref))
    }

    fn check_can_resume_at(&mut self, node: &NodeRef<S>) -> GExecResult<bool> {
        self.core.check_can_resume_at(self.proc_id, node)
    }
}

/// Whether a stage's plan, if it has one, covers a link.
fn check_planned<S: GChainSpec>(
    plans: &[StagePlan<S>],
    proc_id: ProcId,
    lref: &LinkRef<S>,
) -> bool {
    plans
        .iter()
        .any(|plan| plan.proc_id == proc_id && plan.path.links().contains(lref))
}

/// Works out which committed links each stage has to process again, in
/// canonical order.
///
/// A stage that has never been initialized has no history to redo, and no
/// stage redoes anything from before the node it was initialized at.
pub(crate) fn plan_reprocessing<
    S: GChainSpec,
    P: ChainProvider<Spec = S>,
    X: ExecutorStore<Spec = S>,
>(
    core: &ExecutorCore<S, P, X>,
) -> GExecResult<Vec<StagePlan<S>>> {
    let tracking = core.tracking();
    let mut plans: Vec<StagePlan<S>> = Vec::new();

    for proc_id in core.proc_ids() {
        let Some(floor) = tracking.get_stage_floor(proc_id) else {
            continue;
        };

        let mut judge = StageJudge {
            core,
            proc_id,
            plans: &plans,
        };

        match core.find_resume_path(floor, &mut judge)? {
            ResumeSearch::UpToDate => {}
            ResumeSearch::ResumeAt(path) => plans.push(StagePlan { proc_id, path }),
            ResumeSearch::NoResumePoint => return Err(GExecError::NoResumePoint(proc_id)),
        }
    }

    Ok(plans)
}

/// A committed link a stage rejected while reprocessing.
struct Rejection<S: GChainSpec> {
    lref: LinkRef<S>,
    origin: NodeRef<S>,
    target: NodeRef<S>,

    /// The link's position in the stage's plan.
    plan_pos: usize,
}

/// Rolls a stage back to where its plan resumes, runs it over the plan's
/// links, and commits it back up to the committed node.
///
/// If the stage rejects a link, it's left committed up to the link's origin
/// and the rejection is returned.
fn reprocess_stage<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>(
    core: &mut ExecutorCore<S, P, X>,
    plan: &StagePlan<S>,
) -> GExecResult<Option<Rejection<S>>> {
    let proc_id = plan.proc_id;
    let committed = core.committed_path().clone();

    let resume_node = plan.path.base_node();
    let resume_pos = committed
        .get_node_index(resume_node)
        .ok_or_else(|| GExecError::node_not_on_committed_path(resume_node))?;
    let stage_node = core
        .tracking()
        .get_stage_node(proc_id)
        .cloned()
        .expect("gchain: planned stage not initialized");
    let stage_pos = committed
        .get_node_index(&stage_node)
        .ok_or_else(|| GExecError::stage_diverged(proc_id, &stage_node))?;

    // A stage that was already behind where it resumes stays where it is,
    // and has what's in between committed along with the rest.
    let rewind_pos = resume_pos.min(stage_pos);
    let rewind_node = &committed.nodes()[rewind_pos];
    rewind_stage(core, proc_id, rewind_node)?;

    for (plan_pos, lref) in plan.path.links().iter().enumerate() {
        let origin_pos = resume_pos + plan_pos;
        let pre_path = committed.slice(rewind_pos, origin_pos);
        let outcome = run_link(core, lref, &pre_path, &[proc_id])?;

        if outcome.is_rejected() {
            let origin = &committed.nodes()[origin_pos];
            level_stage(core, proc_id, origin)?;
            return Ok(Some(Rejection {
                lref: lref.clone(),
                origin: origin.clone(),
                target: committed.nodes()[origin_pos + 1].clone(),
                plan_pos,
            }));
        }
    }

    level_stage(core, proc_id, committed.terminal_node())?;
    Ok(None)
}

/// Rolls the pipeline back to the origin of a committed link a stage
/// rejected, then forgets the link and everything only reachable through
/// it, returning what was forgotten.
///
/// The rejecting stage is at the origin already.  Stages that were never
/// initialized have nothing to roll back.
fn roll_back_rejected<S: GChainSpec, P: ChainProvider<Spec = S>, X: ExecutorStore<Spec = S>>(
    core: &mut ExecutorCore<S, P, X>,
    rejecting: ProcId,
    rejection: &Rejection<S>,
) -> GExecResult<Vec<LinkRef<S>>> {
    let committed = core.committed_path().clone();
    let origin = &rejection.origin;
    let origin_pos = committed
        .get_node_index(origin)
        .ok_or_else(|| GExecError::node_not_on_committed_path(origin))?;

    let proc_ids: Vec<_> = core.proc_ids().rev().collect();
    for proc_id in proc_ids {
        if proc_id == rejecting {
            continue;
        }
        let Some(node) = core.tracking().get_stage_node(proc_id).cloned() else {
            continue;
        };

        let pos = committed
            .get_node_index(&node)
            .ok_or_else(|| GExecError::stage_diverged(proc_id, &node))?;
        if pos > origin_pos {
            rewind_stage(core, proc_id, origin)?;
        }
    }

    core.truncate_committed_to(origin)?;
    core.forget_link(&rejection.lref)?;
    let mut dropped = vec![rejection.lref.clone()];
    dropped.extend(core.sweep_from(&rejection.target)?);
    core.evict_to_committed();

    Ok(dropped)
}
