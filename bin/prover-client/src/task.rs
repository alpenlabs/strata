use std::collections::{HashMap, HashSet};

use strata_primitives::proof::ProofKey;

use crate::{errors::ProvingTaskError, status::ProvingTaskStatus};

/// Manages tasks and their states for proving operations.
#[derive(Debug, Clone)]
pub struct TaskTracker {
    /// A map of task IDs to their statuses.
    tasks: HashMap<ProofKey, ProvingTaskStatus>,
    /// Count of the tasks that are in progress
    in_progress_tasks: usize,
}

impl TaskTracker {
    /// Creates a new `TaskTracker` instance.
    pub fn new() -> Self {
        TaskTracker {
            tasks: HashMap::new(),
            in_progress_tasks: 0,
        }
    }

    /// Clears all tasks from the tracker.
    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
    }

    pub fn in_progress_tasks_count(&self) -> usize {
        self.in_progress_tasks
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
        deps: Vec<ProofKey>,
    ) -> Result<(), ProvingTaskError> {
        if self.tasks.contains_key(&id) {
            return Err(ProvingTaskError::TaskAlreadyFound(id));
        }

        for dep in &deps {
            if !self.tasks.contains_key(dep) {
                return Err(ProvingTaskError::DependencyNotFound(*dep));
            }
        }

        let status = if deps.is_empty() {
            ProvingTaskStatus::Pending
        } else {
            ProvingTaskStatus::WaitingForDependencies(HashSet::from_iter(deps))
        };

        self.tasks.insert(id, status);

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
                self.in_progress_tasks += 1;
            }

            if new_status == ProvingTaskStatus::Completed {
                self.in_progress_tasks -= 1;
                // Resolve dependencies if a task is completed
                for task_status in self.tasks.values_mut() {
                    if let ProvingTaskStatus::WaitingForDependencies(deps) = task_status {
                        deps.remove(&id);
                        if deps.is_empty() {
                            task_status.transition(ProvingTaskStatus::Pending)?;
                        }
                    }
                }
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
}

#[cfg(test)]
mod tests {
    use strata_state::l1::L1BlockId;
    use strata_test_utils::ArbitraryGenerator;

    use strata_primitives::proof::{ProofId, ProofZkVmHost};
    use strata_state::l1::L1BlockId;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;

    // Helper function to generate test L1 block IDs
    fn gen_task_with_deps(n: u64) -> (ProofKey, Vec<ProofKey>) {
        let mut deps = Vec::with_capacity(n as usize);
        let host = ProofZkVmHost::Native;
        let gen = ArbitraryGenerator::new();

        let start: L1BlockId = gen.generate();
        let end: L1BlockId = gen.generate();
        for _ in 0..n {
            let blkid: L1BlockId = gen.generate();
            let id = ProofId::BtcBlockspace(blkid);
            let key = ProofKey::new(id, host);
            deps.push(key);
        }

        let id = ProofId::L1Batch(start, end);
        let key = ProofKey::new(id, host);

        (key, deps)
    }

    #[test]
    fn test_insert_task_no_dependencies() {
        let mut tracker = TaskTracker::new();
        let id = ProofKey::BtcBlockspace(L1BlockId::default());

        tracker.insert_task(id, vec![]).unwrap();
        assert!(
            matches!(tracker.get_task(id), Ok(&ProvingTaskStatus::Pending)),
            "Task with no dependencies should be Pending"
        );
    }

    #[test]
    fn test_insert_task_with_dependencies() {
        let mut tracker = TaskTracker::new();
        let l1_blkids = gen_l1_block_ids(2);
        let id = ProofKey::L1Batch(l1_blkids[0], l1_blkids[1]);
        let deps = vec![
            ProofKey::BtcBlockspace(l1_blkids[0]),
            ProofKey::BtcBlockspace(l1_blkids[1]),
        ];

        for dep in &deps {
            tracker.insert_task(*dep, vec![]).unwrap();
        }
        tracker.insert_task(id, deps.clone()).unwrap();
        assert!(
            matches!(
                tracker.get_task(id),
                Ok(&ProvingTaskStatus::WaitingForDependencies(_))
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
        let l1_blkids = gen_l1_block_ids(2);
        let id = ProofKey::L1Batch(l1_blkids[0], l1_blkids[1]);
        let deps = vec![
            ProofKey::BtcBlockspace(l1_blkids[0]),
            ProofKey::BtcBlockspace(l1_blkids[1]),
        ];
        for dep in &deps {
            tracker.insert_task(*dep, vec![]).unwrap();
        }
        tracker.insert_task(id, deps.clone()).unwrap();

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
