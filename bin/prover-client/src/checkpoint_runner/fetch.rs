use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use tracing::error;

use super::error::CheckpointResult;
use crate::checkpoint_runner::error::CheckpointError;

/// Fetches the latest checkpoint index from the sequencer client.
pub async fn fetch_latest_checkpoint_index(sequencer_client: &HttpClient) -> CheckpointResult<u64> {
    sequencer_client
        .request::<Option<u64>, _>("strata_getLatestCheckpointIndex", rpc_params![])
        .await
        .map_err(|e| {
            error!("Failed to fetch current checkpoint index: {e}");
            CheckpointError::FetchError(e.to_string())
        })?
        .ok_or_else(|| {
            error!("No checkpoint index returned from sequencer");
            CheckpointError::CheckpointNotFound(0)
        })
}
