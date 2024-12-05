use std::sync::Arc;

use jsonrpsee::{
    core::{client::ClientT, params::ArrayParams},
    http_client::HttpClient,
    rpc_params,
};
use strata_rpc_types::HexBytes;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    config::CHECKPOINT_POLL_INTERVAL,
    dispatcher::TaskDispatcher,
    primitives::tasks_scheduler::ProvingTaskStatus,
    proving_ops::checkpoint_ops::{CheckpointOperations, CheckpointOpsParam},
    task::TaskTracker,
};

/// Continuously checks for the latest checkpoint index.
/// Dispatches tasks when a new checkpoint is detected and discards the current proving tasks.
pub async fn start_checkpoints_task(
    sequencer_client: HttpClient,
    ckp_task_dispatcher: TaskDispatcher<CheckpointOperations>,
    task_tracker: Arc<TaskTracker>,
) {
    info!("Checkpoint runner started");
    let mut to_fetch_idx = 0;
    let mut ticker = interval(Duration::from_secs(CHECKPOINT_POLL_INTERVAL));
    let mut current_task_id = Uuid::default();

    loop {
        ticker.tick().await;

        let new_checkpoint_idx =
            fetch_latest_checkpoint_index(&sequencer_client, to_fetch_idx).await;

        if new_checkpoint_idx >= to_fetch_idx {
            match handle_new_checkpoint(new_checkpoint_idx, &ckp_task_dispatcher, &task_tracker)
                .await
            {
                Ok(task_id) => {
                    current_task_id = task_id;
                    to_fetch_idx = new_checkpoint_idx + 1;
                }
                Err(_) => {
                    warn!(
                        "Failed to handle new checkpoint idx {:?}",
                        new_checkpoint_idx
                    );
                }
            }
        }

        if current_task_id != Uuid::default() {
            check_and_submit_proof(
                to_fetch_idx,
                &sequencer_client,
                &task_tracker,
                &mut current_task_id,
            )
            .await;
        }
    }
}

async fn fetch_latest_checkpoint_index(
    sequencer_client: &HttpClient,
    current_checkpoint_idx: u64,
) -> u64 {
    match sequencer_client
        .request("strata_getLatestCheckpointIndex", rpc_params![])
        .await
    {
        Ok(Some(idx)) => idx,
        Ok(None) => {
            error!("Failed to fetch current checkpoint");
            current_checkpoint_idx
        }
        Err(e) => {
            error!("Failed to fetch current checkpoint index: {}", e);
            current_checkpoint_idx
        }
    }
}

async fn handle_new_checkpoint(
    new_checkpoint_idx: u64,
    ckp_task_dispatcher: &TaskDispatcher<CheckpointOperations>,
    task_tracker: &Arc<TaskTracker>,
) -> Result<Uuid, ()> {
    // Discard ongoing tasks
    task_tracker.clear_tasks().await;

    // Create proving task for the new checkpoint
    match ckp_task_dispatcher
        .create_task(CheckpointOpsParam::CheckPointIndex(new_checkpoint_idx))
        .await
    {
        Ok(task_id) => {
            info!("Updated to new checkpoint index: {}", new_checkpoint_idx);
            Ok(task_id)
        }
        Err(e) => {
            error!("Failed to create checkpoint task: {}", e);
            Err(())
        }
    }
}

async fn check_and_submit_proof(
    current_idx: u64,
    sequencer_client: &HttpClient,
    task_tracker: &Arc<TaskTracker>,
    current_task_id: &mut Uuid,
) {
    if let Some(proving_task) = task_tracker.get_task(*current_task_id).await {
        if proving_task.status == ProvingTaskStatus::Completed {
            match &proving_task.proof {
                Some(proof) => {
                    let proof_bytes = HexBytes::from(proof.receipt().proof().as_bytes());
                    info!(
                        "Sending checkpoint proof: {:?} ckp id: {:?} to the sequencer",
                        current_task_id, current_idx
                    );
                    match sequencer_client
                        .request::<(), ArrayParams>(
                            "strataadmin_submitCheckpointProof",
                            rpc_params![current_idx, proof_bytes],
                        )
                        .await
                    {
                        Ok(_) => {
                            *current_task_id = Uuid::default();
                        }
                        Err(e) => {
                            error!("Failed to submit checkpoint proof: {}", e);
                        }
                    }
                }
                None => {
                    warn!(
                        "Proving task {:?} is completed but proof is missing.",
                        current_task_id
                    );
                }
            }
        }
    }
}
