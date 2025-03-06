use std::sync::Arc;

use strata_rocksdb::prover::db::ProofDb;
use tokio::{
    sync::Mutex,
    time::{interval, Duration},
};
use tracing::{error, info, warn};

use crate::{
    checkpoint_runner::fetch::fetch_latest_checkpoint_index,
    operators::{checkpoint::CheckpointOperator, ProvingOp},
    task_tracker::TaskTracker,
};

const CHECKPOINT_POLL_INTERVAL: u64 = 10;

/// Holds the current checkpoint index for the runner to track progress.
#[derive(Default)]
struct CheckpointRunnerState {
    pub current_checkpoint_idx: Option<u64>,
}

/// Periodically polls for the latest checkpoint index and updates the current index.
/// Dispatches tasks when a new checkpoint is detected.
pub async fn checkpoint_proof_runner(
    operator: CheckpointOperator,
    task_tracker: Arc<Mutex<TaskTracker>>,
    db: Arc<ProofDb>,
) {
    info!("Checkpoint runner started");
    let mut ticker = interval(Duration::from_secs(CHECKPOINT_POLL_INTERVAL));
    let mut runner_state = CheckpointRunnerState::default();

    loop {
        ticker.tick().await;

        if let Err(e) = process_checkpoint(&operator, &task_tracker, &db, &mut runner_state).await {
            error!(err = ?e, "error processing checkpoint");
        }
    }
}

async fn process_checkpoint(
    operator: &CheckpointOperator,
    task_tracker: &Arc<Mutex<TaskTracker>>,
    db: &Arc<ProofDb>,
    runner_state: &mut CheckpointRunnerState,
) -> anyhow::Result<()> {
    let latest_checkpoint = fetch_latest_checkpoint_index(operator.cl_client()).await;

    let checkpoint_idx = match latest_checkpoint {
        Ok(idx) => idx,
        Err(e) => {
            warn!(error = ?e, "Unable to fetch latest checkpoint index");
            return Ok(());
        }
    };

    if !should_update_checkpoint(runner_state.current_checkpoint_idx, checkpoint_idx) {
        warn!("Fetched checkpoint {checkpoint_idx} is not newer than current checkpoint");
        return Ok(());
    }

    operator
        .create_task(checkpoint_idx, task_tracker.clone(), db)
        .await?;
    runner_state.current_checkpoint_idx = Some(checkpoint_idx);

    Ok(())
}

fn should_update_checkpoint(current: Option<u64>, new: u64) -> bool {
    current.is_none_or(|current| new > current)
}
