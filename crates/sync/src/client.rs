use std::cmp::min;

use alpen_express_rpc_api::AlpenApiClient;
use alpen_express_rpc_types::NodeSyncStatus;
use alpen_express_state::{block::L2BlockBundle, id::L2BlockId};
use futures::stream::{self, Stream, StreamExt};
use tracing::{debug, error};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("missing block: {0}")]
    MissingBlock(L2BlockId),
    #[error("failed to deserialize block: {0}")]
    DeserializationError(String),
    #[error("network error: {0}")]
    NetworkError(String),
}

#[async_trait::async_trait]
pub trait SyncClient {
    async fn get_sync_status(&self) -> Result<NodeSyncStatus, ClientError>;

    fn get_blocks_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> impl Stream<Item = L2BlockBundle>;

    async fn get_block_by_id(
        &self,
        block_id: &L2BlockId,
    ) -> Result<Option<L2BlockBundle>, ClientError>;
}

pub struct RpcSyncPeer<RPC: AlpenApiClient + Send + Sync> {
    rpc_client: RPC,
    download_batch_size: usize,
}

impl<RPC: AlpenApiClient + Send + Sync> RpcSyncPeer<RPC> {
    pub fn new(rpc_client: RPC, download_batch_size: usize) -> Self {
        Self {
            rpc_client,
            download_batch_size,
        }
    }

    async fn get_blocks(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<L2BlockBundle>, ClientError> {
        let bytes = self
            .rpc_client
            .sync_blocks(start_height, end_height)
            .await
            .map_err(|e| ClientError::NetworkError(e.to_string()))?;

        match borsh::from_slice(&bytes.0) {
            Ok(blocks) => Ok(blocks),
            Err(err) => Err(ClientError::DeserializationError(err.to_string())),
        }
    }
}

#[async_trait::async_trait]
impl<RPC: AlpenApiClient + Send + Sync> SyncClient for RpcSyncPeer<RPC> {
    async fn get_sync_status(&self) -> Result<NodeSyncStatus, ClientError> {
        let status = self
            .rpc_client
            .sync_status()
            .await
            .map_err(|e| ClientError::NetworkError(e.to_string()))?;
        Ok(status)
    }

    fn get_blocks_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> impl Stream<Item = L2BlockBundle> {
        let block_ranges = (start_height..=end_height)
            .step_by(self.download_batch_size)
            .map(move |s| (s, min(self.download_batch_size as u64 + s - 1, end_height)));

        stream::unfold(block_ranges, |mut block_ranges| async {
            let (start_height, end_height) = block_ranges.next()?;
            match self.get_blocks(start_height, end_height).await {
                Ok(blocks) => Some((stream::iter(blocks), block_ranges)),
                Err(err) => {
                    error!("failed to get blocks: {err}");
                    None
                }
            }
        })
        .flatten()
    }

    async fn get_block_by_id(
        &self,
        block_id: &L2BlockId,
    ) -> Result<Option<L2BlockBundle>, ClientError> {
        let bytes = self
            .rpc_client
            .sync_block_by_id(*block_id)
            .await
            .map_err(|e| ClientError::NetworkError(e.to_string()))?
            .ok_or(ClientError::MissingBlock(*block_id))?;

        match borsh::from_slice(&bytes.0) {
            Ok(blocks) => Ok(blocks),
            Err(err) => {
                error!("failed to deserialize blocks: {err}");
                Err(ClientError::DeserializationError(err.to_string()))
            }
        }
    }
}
