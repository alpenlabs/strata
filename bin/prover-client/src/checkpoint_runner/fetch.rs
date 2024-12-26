use jsonrpsee::{core::client::ClientT, http_client::HttpClient, rpc_params};
use tracing::error;

/// Fetches the latest checkpoint index from the sequencer client.
pub async fn fetch_latest_checkpoint_index(sequencer_client: &HttpClient) -> anyhow::Result<u64> {
    match sequencer_client
        .request("strata_getLatestCheckpointIndex", rpc_params![])
        .await
    {
        Ok(Some(idx)) => Ok(idx),
        Ok(None) => {
            error!("Failed to fetch current checkpoint");
            Err(anyhow::anyhow!("Failed to fetch current checkpoint"))
        }
        Err(e) => {
            error!("Failed to fetch current checkpoint index: {}", e);
            Err(anyhow::anyhow!(
                "Failed to fetch current checkpoint index: {}",
                e
            ))
        }
    }
}
