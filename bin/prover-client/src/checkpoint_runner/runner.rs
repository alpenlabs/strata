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
    let mut ticker = interval(Duration::from_secs(CHECKPOINT_POLL_INTERVAL));
    let mut current_checkpoint_idx: Option<u64> = None;

    loop {
        ticker.tick().await;

        if let Err(e) =
            process_checkpoint(&operator, &task_tracker, &db, &mut current_checkpoint_idx).await
        {
            error!("Error processing checkpoint: {e:?}");
        }
    }
}

async fn process_checkpoint(
    operator: &CheckpointOperator,
    task_tracker: &Arc<Mutex<TaskTracker>>,
    db: &Arc<ProofDb>,
    current_checkpoint_idx: &mut Option<u64>,
) -> anyhow::Result<()> {
    let new_checkpoint = fetch_latest_checkpoint_index(operator.cl_client()).await?;

    if !should_update_checkpoint(*current_checkpoint_idx, new_checkpoint) {
        info!(
           "Fetched checkpoint {new_checkpoint} is not newer than current {current_checkpoint_idx:?}"
       );
        return Ok(());
    }

    operator
        .create_task(new_checkpoint, task_tracker.clone(), db)
        .await?;
    *current_checkpoint_idx = Some(new_checkpoint);

    Ok(())
}

fn should_update_checkpoint(current: Option<u64>, new: u64) -> bool {
    current.is_none_or(|current| new > current)
}
