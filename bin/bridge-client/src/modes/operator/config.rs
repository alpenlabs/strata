//! Defines config for the bridge-client in `Operator` mode.

use super::constants::DEFAULT_DUTY_RETRY_COUNT;

/// Config for [`TaskManager`](super::task_manager::TaskManager).
pub(super) struct TaskConfig {
    pub(super) max_retry_count: u32,
}

impl TaskConfig {
    pub(super) fn new(max_retry_count: Option<u32>) -> Self {
        let max_retry_count = max_retry_count.unwrap_or(DEFAULT_DUTY_RETRY_COUNT);

        Self { max_retry_count }
    }
}
