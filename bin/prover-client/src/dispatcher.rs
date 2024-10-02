use std::{fmt::Debug, sync::Arc, time::Duration};

use tokio::time;
use tracing::{debug, error};
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
    last_param: O::Params,
    interval: Duration,
}

impl<O> TaskDispatcher<O>
where
    O: ProvingOperations + Clone + Send + Sync + 'static,
    O::Params: Debug + Clone,
{
    /// Creates a new task dispatcher.
    pub fn new(
        operations: O,
        task_tracker: Arc<TaskTracker>,
        last_param: O::Params,
        interval: Duration,
    ) -> Self {
        Self {
            operations,
            task_tracker,
            last_param,
            interval,
        }
    }

    /// Starts listening for new blocks and processes them automatically.
    pub async fn start(&mut self) {
        let mut ticker = time::interval(self.interval);
        loop {
            match self.create_task(self.last_param.clone()).await {
                Ok(_) => {
                    self.update_last_param();
                }
                Err(e) => {
                    error!("Error processing block {:?}: {:?}", self.last_param, e);
                }
            }
            ticker.tick().await;
        }
    }

    /// Creates a proving task for the given params.
    pub async fn create_task(&self, param: O::Params) -> Result<Uuid, ProvingTaskError> {
        debug!("Creating proving task for block {:?}", param);
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
