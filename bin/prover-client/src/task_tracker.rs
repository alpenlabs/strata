use std::collections::{HashMap, HashSet};

use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofContext, ProofKey, ProofZkVm};
use strata_rocksdb::prover::db::ProofDb;
use tracing::info;

use crate::{errors::ProvingTaskError, status::ProvingTaskStatus};

/// Manages tasks and their states for proving operations.
#[derive(Debug, Clone)]
pub struct TaskTracker {
    /// A map of task IDs to their statuses.
    tasks: HashMap<ProofKey, ProvingTaskStatus>,
    /// A map of task IDs to their dependencies that have not yet been proven.
    pending_dependencies: HashMap<ProofKey, HashSet<ProofKey>>,
    /// Count of the tasks that are in progress
    in_progress_tasks: HashMap<ProofZkVm, usize>,
    /// List of ZkVm for which the task is created
    vms: Vec<ProofZkVm>,
}

impl TaskTracker {
    /// Creates a new `TaskTracker` instance.
    pub fn new() -> Self {
        let mut vms = vec![];

        #[cfg(feature = "sp1")]
        {
            vms.push(ProofZkVm::SP1);
        }

        #[cfg(feature = "risc0")]
        {
            vms.push(ProofZkVm::Risc0);
        }

        #[cfg(all(not(feature = "risc0"), not(feature = "sp1")))]
        {
            vms.push(ProofZkVm::Native);
        }

        TaskTracker {
            tasks: HashMap::new(),
            pending_dependencies: HashMap::new(),
            in_progress_tasks: HashMap::new(),
            vms,
        }
    }

    pub fn get_in_progress_tasks(&self) -> &HashMap<ProofZkVm, usize> {
        &self.in_progress_tasks
    }

    pub fn create_tasks(
        &mut self,
        proof_id: ProofContext,
        deps: Vec<ProofContext>,
        db: &ProofDb,
    ) -> Result<Vec<ProofKey>, ProvingTaskError> {
        info!(?proof_id, "Creating task for");
        let mut tasks = Vec::with_capacity(self.vms.len());
        // Insert tasks for each configured host
        let vms = &self.vms.clone();
        for host in vms {
            let task = ProofKey::new(proof_id, *host);
            tasks.push(task);
            let dep_tasks: Vec<_> = deps.iter().map(|&dep| ProofKey::new(dep, *host)).collect();
            self.insert_task(task, &dep_tasks, db)?;
        }

        Ok(tasks)
    }

    /// Inserts a new task with the given dependencies.
    ///
    /// - If no dependencies are provided, the task is marked as `Pending`.
    /// - If dependencies are provided, the task is marked as `WaitingForDependencies`.
    ///
    /// Returns an error if the task already exists.
    pub fn insert_task(
        &mut self,
        id: ProofKey,
        deps: &[ProofKey],
        db: &ProofDb,
    ) -> Result<(), ProvingTaskError> {
        if self.tasks.contains_key(&id) {
            return Err(ProvingTaskError::TaskAlreadyFound(id));
        }

        // Gather dependencies that are not completed
        let mut pending_deps = Vec::with_capacity(deps.len());
        for &dep in deps {
            let proof = db
                .get_proof(&dep)
                .map_err(ProvingTaskError::DatabaseError)?;
            match proof {
                Some(_) => {}
                None => {
                    pending_deps.push(dep);
                }
            }
        }

        if pending_deps.is_empty() {
            self.tasks.insert(id, ProvingTaskStatus::Pending);
        } else {
            self.pending_dependencies
                .insert(id, HashSet::from_iter(pending_deps));
            self.tasks
                .insert(id, ProvingTaskStatus::WaitingForDependencies);
        };

        Ok(())
    }

    /// Retrieves the status of a task by its ID.
    ///
    /// Returns an error if the task does not exist.
    pub fn get_task(&self, id: ProofKey) -> Result<&ProvingTaskStatus, ProvingTaskError> {
        self.tasks
            .get(&id)
            .ok_or(ProvingTaskError::TaskNotFound(id))
    }

    /// Updates the status of a task.
    ///
    /// - Allows valid transitions as per the state machine.
    /// - Automatically resolves dependencies if a task is completed.
    ///
    /// Returns an error for invalid transitions or if the task does not exist.
    pub fn update_status(
        &mut self,
        id: ProofKey,
        new_status: ProvingTaskStatus,
    ) -> Result<(), ProvingTaskError> {
        if let Some(status) = self.tasks.get_mut(&id) {
            // Check for valid status transitions
            status.transition(new_status.clone())?;

            if new_status == ProvingTaskStatus::ProvingInProgress {
                // Increment value if key exists, or insert with a default value of 1
                *self.in_progress_tasks.entry(*id.host()).or_insert(0) += 1;
            }

            if new_status == ProvingTaskStatus::Completed {
                // Decrement value if key exists, or insert with a default value of 1
                *self.in_progress_tasks.entry(*id.host()).or_insert(0) -= 1;

                // Resolve dependencies for other tasks
                let mut tasks_to_update = vec![];
                for (dependent_task, deps) in self.pending_dependencies.iter_mut() {
                    if deps.remove(&id) && deps.is_empty() {
                        tasks_to_update.push(*dependent_task);
                    }
                }

                for task in tasks_to_update {
                    self.pending_dependencies.remove(&task);
                    if let Some(task_status) = self.tasks.get_mut(&task) {
                        task_status.transition(ProvingTaskStatus::Pending)?;
                    }
                }

                self.tasks.remove(&id);
            }
            Ok(())
        } else {
            Err(ProvingTaskError::TaskNotFound(id))
        }
    }

    /// Filters and retrieves a list of `ProofKey` references for tasks whose status
    /// matches the given filter function.
    ///
    /// # Example
    ///
    /// ```rust
    /// let task_tracker = TaskTracker::new();
    /// let pending_tasks =
    ///     task_tracker.get_tasks_by_status(|status| matches!(status, ProvingTaskStatus::Pending));
    /// ```
    pub fn get_tasks_by_status<F>(&self, filter_fn: F) -> Vec<ProofKey>
    where
        F: Fn(&ProvingTaskStatus) -> bool,
    {
        self.tasks
            .iter()
            .filter_map(|(proof_key, task)| {
                if filter_fn(task) {
                    Some(*proof_key) // Only return the `proof_key` if the task matches the filter
                } else {
                    None
                }
            })
            .collect()
    }

    /// Generates a report of task statuses and their counts across all tasks.
    pub fn generate_report(&self) -> HashMap<String, usize> {
        let mut report: HashMap<String, usize> = HashMap::new();

        for status in self.tasks.values() {
            *report.entry(format!("{:?}", status)).or_insert(0) += 1;
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use strata_primitives::proof::{ProofContext, ProofZkVm};
    use strata_rocksdb::test_utils::get_rocksdb_tmp_instance_for_prover;
    use strata_state::l1::L1BlockId;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;

    // Helper function to generate test L1 block IDs
    fn gen_task_with_deps(n: u64) -> (ProofKey, Vec<ProofKey>) {
        let mut deps = Vec::with_capacity(n as usize);
        let host = ProofZkVm::Native;
        let mut gen = ArbitraryGenerator::new();

        let start: L1BlockId = gen.generate();
        let end: L1BlockId = gen.generate();
        for _ in 0..n {
            let blkid: L1BlockId = gen.generate();
            let id = ProofContext::BtcBlockspace(blkid);
            let key = ProofKey::new(id, host);
            deps.push(key);
        }

        let id = ProofContext::L1Batch(start, end);
        let key = ProofKey::new(id, host);

        (key, deps)
    }

    fn setup_db() -> ProofDb {
        let (db, db_ops) = get_rocksdb_tmp_instance_for_prover().unwrap();
        ProofDb::new(db, db_ops)
    }

    #[test]
    fn test_insert_task_no_dependencies() {
        let mut tracker = TaskTracker::new();
        let (id, _) = gen_task_with_deps(0);
        let db = setup_db();

        tracker.insert_task(id, &[], &db).unwrap();
        assert!(
            matches!(tracker.get_task(id), Ok(&ProvingTaskStatus::Pending)),
            "Task with no dependencies should be Pending"
        );
    }

    #[test]
    fn test_insert_task_with_dependencies() {
        let mut tracker = TaskTracker::new();
        let (id, deps) = gen_task_with_deps(2);
        let db = setup_db();

        for dep in &deps {
            tracker.insert_task(*dep, &[], &db).unwrap();
        }
        tracker.insert_task(id, &deps.clone(), &db).unwrap();
        assert!(
            matches!(
                tracker.get_task(id),
                Ok(&ProvingTaskStatus::WaitingForDependencies)
            ),
            "Task with dependencies should be WaitingForDependencies"
        );
    }

    #[test]
    fn test_task_not_found_error() {
        let mut tracker = TaskTracker::new();
        let (id, _) = gen_task_with_deps(0);

        let result = tracker.update_status(id, ProvingTaskStatus::Pending);
        assert!(matches!(result, Err(ProvingTaskError::TaskNotFound(_))));
    }

    #[test]
    fn test_dependency_resolution() {
        let mut tracker = TaskTracker::new();
        let (id, deps) = gen_task_with_deps(2);
        let db = setup_db();

        for dep in &deps {
            tracker.insert_task(*dep, &[], &db).unwrap();
        }
        tracker.insert_task(id, &deps, &db).unwrap();

        for dep in &deps {
            tracker
                .update_status(*dep, ProvingTaskStatus::ProvingInProgress)
                .and_then(|_| tracker.update_status(*dep, ProvingTaskStatus::Completed))
                .unwrap();
        }
        assert!(
            matches!(tracker.get_task(id), Ok(&ProvingTaskStatus::Pending)),
            "Task should become Pending after all dependencies are resolved"
        );
    }
}
