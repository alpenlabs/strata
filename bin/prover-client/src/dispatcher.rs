use std::{sync::Arc, time::Duration};

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
    last_block: u64,
    interval: Duration,
}

impl<O> TaskDispatcher<O>
where
    O: ProvingOperations + Clone + Send + Sync + 'static,
{
    /// Creates a new task dispatcher.
    pub fn new(
        operations: O,
        task_tracker: Arc<TaskTracker>,
        start_block: u64,
        interval: Duration,
    ) -> Self {
        Self {
            operations,
            task_tracker,
            last_block: start_block,
            interval,
        }
    }

    /// Starts listening for new blocks and processes them automatically.
    pub async fn start(&mut self) {
        let mut ticker = time::interval(self.interval);
        loop {
            match self.create_task(self.last_block).await {
                Ok(_) => {
                    self.last_block += 1;
                }
                Err(e) => {
                    error!("Error processing block {}: {:?}", self.last_block, e);
                }
            }
            ticker.tick().await;
        }
    }

    /// Creates a proving task for the given block number.
    pub async fn create_task(&self, block_num: u64) -> Result<Uuid, ProvingTaskError> {
        debug!("Creating proving task for block {}", block_num);
        let input = self.operations.fetch_input(block_num).await.map_err(|e| {
            ProvingTaskError::FetchInputError {
                block_num,
                task_type: self.operations.block_type(),
                source: e,
            }
        })?;
        self.operations
            .append_task(self.task_tracker.clone(), input)
            .await
    }

    pub fn task_tracker(&self) -> Arc<TaskTracker> {
        self.task_tracker.clone()
    }
}
