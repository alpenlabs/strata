use std::{fmt::Debug, sync::Arc};

use tracing::info;
use uuid::Uuid;

use crate::{errors::ProvingTaskError, proving_ops::ops::ProvingOperations, task::TaskTracker};

/// Generic dispatcher for block proving tasks.
#[derive(Debug, Clone)]
pub struct TaskDispatcher<O>
where
    O: ProvingOperations,
{
    operations: O,
    task_tracker: Arc<TaskTracker>,
}

impl<O> TaskDispatcher<O>
where
    O: ProvingOperations + Clone + Send + Sync + 'static,
    O::Params: Debug + Clone,
{
    /// Creates a new task dispatcher.
    pub fn new(operations: O, task_tracker: Arc<TaskTracker>) -> Self {
        Self {
            operations,
            task_tracker,
        }
    }

    /// Creates a proving task for the given params.
    pub async fn create_task(&self, param: O::Params) -> Result<Uuid, ProvingTaskError> {
        info!(
            "Creating proving task for block {:?} {:?}",
            param,
            self.operations.block_type()
        );
        let input = self
            .operations
            .fetch_input(param.clone())
            .await
            .map_err(|e| ProvingTaskError::FetchInput {
                param: format!("{:?}", param),
                task_type: self.operations.block_type(),
                source: e,
            })?;
        self.operations
            .append_task(self.task_tracker.clone(), input)
            .await
    }

    pub fn task_tracker(&self) -> Arc<TaskTracker> {
        self.task_tracker.clone()
    }

    fn update_last_param(&mut self) {
        todo!("need to do this")
    }
}
