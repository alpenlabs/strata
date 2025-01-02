use std::collections::HashSet;

use strata_primitives::proof::ProofKey;

use crate::errors::ProvingTaskError;

/// Represents the status of a proving task.
///
/// ## State Transitions
///
/// - `WaitingForDependencies` -> `Pending`: When all dependencies are resolved.
/// - `Pending` -> `ProvingInProgress`: When the proving task starts.
/// - `ProvingInProgress` -> `Completed`: When the proving task completes successfully.
/// - Any state -> `Failed`: If the task fails at any point.
#[derive(Debug, Clone, PartialEq)]
pub enum ProvingTaskStatus {
    /// Waiting for dependencies to be resolved.
    WaitingForDependencies(HashSet<ProofKey>), // TODO: decouple deps from here
    /// Ready to be started
    Pending,
    /// Task is currently being executed.
    ProvingInProgress,
    /// Task has been completed successfully.
    Completed,
    /// Task has failed.
    Failed,
}

impl ProvingTaskStatus {
    /// Attempts to transition the current task status to a new status.
    ///
    /// # Returns
    /// * `Ok(())` if the transition is valid
    /// * `Err(ProvingTaskError::InvalidStatusTransition)` if the transition is not allowed
    pub fn transition(&mut self, target_status: ProvingTaskStatus) -> Result<(), ProvingTaskError> {
        let is_transition_valid = match (self.clone(), &target_status) {
            // Always allow transitioning to Failed
            (_, &ProvingTaskStatus::Failed) => true,

            // Specific allowed state transitions
            (ProvingTaskStatus::Pending, ProvingTaskStatus::ProvingInProgress) => true,
            (ProvingTaskStatus::ProvingInProgress, &ProvingTaskStatus::Completed) => true,

            // Special case: WaitingForDependencies can only become Pending if no dependencies
            (
                ProvingTaskStatus::WaitingForDependencies(dependencies),
                &ProvingTaskStatus::Pending,
            ) => dependencies.is_empty(),

            // All other transitions are invalid
            _ => false,
        };

        if is_transition_valid {
            *self = target_status;
            Ok(())
        } else {
            Err(ProvingTaskError::InvalidStatusTransition(
                self.clone(),
                target_status,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use strata_primitives::proof::{ProofContext, ProofZkVm};
    use strata_state::l1::L1BlockId;
    use strata_test_utils::ArbitraryGenerator;

    use super::*;

    // Helper function to generate test L1 block IDs
    fn gen_deps(n: usize) -> HashSet<ProofKey> {
        let mut deps = HashSet::with_capacity(n);
        let mut gen = ArbitraryGenerator::new();
        for _ in 0..n {
            let st_blkid: L1BlockId = gen.generate();
            let end_blkid: L1BlockId = gen.generate();
            let host = ProofZkVm::Native;
            let id = ProofContext::BtcBlockspace(st_blkid, end_blkid);
            let key = ProofKey::new(id, host);
            deps.insert(key);
        }
        deps
    }

    #[test]
    fn test_transition_to_failed() {
        // Test transitioning to Failed from every possible state
        let test_cases = vec![
            ProvingTaskStatus::Pending,
            ProvingTaskStatus::ProvingInProgress,
            ProvingTaskStatus::Completed,
            ProvingTaskStatus::WaitingForDependencies(HashSet::new()),
            ProvingTaskStatus::Failed,
        ];

        for mut current_status in test_cases {
            let original_status = current_status.clone();
            let result = current_status.transition(ProvingTaskStatus::Failed);

            assert!(
                result.is_ok(),
                "Failed to transition {:?} to Failed",
                original_status
            );
            assert_eq!(
                current_status,
                ProvingTaskStatus::Failed,
                "Status should be Failed after transition from {:?}",
                original_status
            );
        }
    }

    #[test]
    fn test_pending_to_proving_in_progress() {
        let mut status = ProvingTaskStatus::Pending;
        let result = status.transition(ProvingTaskStatus::ProvingInProgress);

        assert!(result.is_ok());
        assert_eq!(status, ProvingTaskStatus::ProvingInProgress);
    }

    #[test]
    fn test_proving_in_progress_to_completed() {
        let mut status = ProvingTaskStatus::ProvingInProgress;
        let result = status.transition(ProvingTaskStatus::Completed);

        assert!(result.is_ok());
        assert_eq!(status, ProvingTaskStatus::Completed);
    }

    #[test]
    fn test_waiting_for_dependencies_to_pending() {
        // Test transitioning from WaitingForDependencies to Pending with empty dependencies
        let mut status = ProvingTaskStatus::WaitingForDependencies(HashSet::new());
        let result = status.transition(ProvingTaskStatus::Pending);

        assert!(result.is_ok());
        assert_eq!(status, ProvingTaskStatus::Pending);

        // Test transitioning from WaitingForDependencies to Pending with non-empty dependencies
        let block_ids = gen_deps(3);
        let mut status_with_deps = ProvingTaskStatus::WaitingForDependencies(block_ids);
        let result_with_deps = status_with_deps.transition(ProvingTaskStatus::Pending);

        assert!(result_with_deps.is_err());
    }

    #[test]
    fn test_invalid_transitions() {
        let invalid_transitions = vec![
            // Completed cannot transition to other states except Failed
            (ProvingTaskStatus::Completed, ProvingTaskStatus::Pending),
            (
                ProvingTaskStatus::Completed,
                ProvingTaskStatus::ProvingInProgress,
            ),
            // ProvingInProgress cannot go back to Pending
            (
                ProvingTaskStatus::ProvingInProgress,
                ProvingTaskStatus::Pending,
            ),
            // Pending cannot go back to WaitingForDependencies
            (
                ProvingTaskStatus::Pending,
                ProvingTaskStatus::WaitingForDependencies(HashSet::new()),
            ),
            // WaitingForDependencies with non-empty deps cannot go to Pending
            (
                ProvingTaskStatus::WaitingForDependencies(gen_deps(2)),
                ProvingTaskStatus::Pending,
            ),
        ];

        for (current_status, target_status) in invalid_transitions {
            let mut status = current_status.clone();
            let result = status.transition(target_status.clone());

            assert!(
                result.is_err(),
                "Transition from {:?} to {:?} should be invalid",
                current_status,
                target_status
            );
        }
    }

    #[test]
    fn test_error_details() {
        let mut status = ProvingTaskStatus::Pending;
        let invalid_target = ProvingTaskStatus::Completed;

        let result = status.transition(invalid_target.clone());

        assert!(result.is_err());

        if let Err(ProvingTaskError::InvalidStatusTransition(from, to)) = result {
            assert_eq!(from, ProvingTaskStatus::Pending);
            assert_eq!(to, ProvingTaskStatus::Completed);
        } else {
            panic!("Expected InvalidStatusTransition error");
        }
    }
}
