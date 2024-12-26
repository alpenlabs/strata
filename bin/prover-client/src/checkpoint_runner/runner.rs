use std::sync::Arc;

use strata_rocksdb::prover::db::ProofDb;
use tokio::{
    sync::Mutex,
    time::{interval, Duration},
};
use tracing::{error, info};

use crate::{
    checkpoint_runner::fetch::fetch_latest_checkpoint_index,
    operators::{checkpoint::CheckpointOperator, ProvingOp},
    task_tracker::TaskTracker,
};

const CHECKPOINT_POLL_INTERVAL: u64 = 10;

/// Periodically polls for the latest checkpoint index and updates the current index.
/// Dispatches tasks when a new checkpoint is detected.
pub async fn checkpoint_proof_runner(
    operator: CheckpointOperator,
    task_tracker: Arc<Mutex<TaskTracker>>,
    db: Arc<ProofDb>,
) {
    info!("Checkpoint runner started");

    let poll_interval = Duration::from_secs(CHECKPOINT_POLL_INTERVAL);
    let mut ticker = interval(poll_interval);
    let mut current_checkpoint_idx: Option<u64> = None;
    loop {
        ticker.tick().await;

        match fetch_latest_checkpoint_index(operator.cl_client()).await {
            Ok(new_checkpoint) => {
                // Determine if we should update the checkpoint
                let should_update =
                    current_checkpoint_idx.is_none_or(|current| new_checkpoint > current);

                if should_update {
                    // Create new proving task
                    if let Err(e) = operator
                        .create_task(new_checkpoint, task_tracker.clone(), &db)
                        .await
                    {
                        error!("Failed to create proving task: {:?}", e);
                        continue;
                    }

                    // Update the checkpoint index
                    current_checkpoint_idx = Some(new_checkpoint);
                } else {
                    info!(
                        "Fetched checkpoint {} is not newer than current {:?}",
                        new_checkpoint, current_checkpoint_idx
                    );
                }
            }
            Err(e) => {
                error!("Failed to fetch the latest checkpoint index: {:?}", e);
            }
        }
    }
}
