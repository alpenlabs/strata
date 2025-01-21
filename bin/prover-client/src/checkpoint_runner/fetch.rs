use jsonrpsee::http_client::HttpClient;
use strata_rpc_api::StrataApiClient;
use tracing::error;

use super::errors::CheckpointResult;
use crate::checkpoint_runner::errors::CheckpointError;

/// Fetches the latest checkpoint index from the sequencer client.
pub async fn fetch_latest_checkpoint_index(cl_client: &HttpClient) -> CheckpointResult<u64> {
    cl_client
        .get_latest_checkpoint_index(None)
        .await
        .map_err(|e| {
            error!(err = ?e, "Failed to fetch current checkpoint index");
            CheckpointError::FetchError(e.to_string())
        })?
        .ok_or_else(|| {
            error!("No checkpoint index returned from sequencer");
            CheckpointError::CheckpointNotFound(0)
        })
}
