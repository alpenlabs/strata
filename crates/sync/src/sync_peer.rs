use alpen_express_rpc_api::{AlpenApiClient, AlpenSyncApiClient};
use alpen_express_rpc_types::NodeSyncStatus;
use alpen_express_state::{block::L2BlockBundle, id::L2BlockId};
use tracing::error;

#[derive(Debug, thiserror::Error)]
pub enum SyncPeerError {
    #[error("missing block: {0}")]
    MissingBlock(L2BlockId),
    #[error("failed to deserialize block: {0}")]
    DeserializationError(String),
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("other: {0}")]
    Other(String),
}

#[async_trait::async_trait]
pub trait SyncPeer {
    async fn fetch_sync_status(&self) -> Result<NodeSyncStatus, SyncPeerError>;

    async fn fetch_blocks_by_height(
        &self,
        height: u64,
    ) -> Result<Vec<L2BlockBundle>, SyncPeerError>;

    async fn fetch_blocks_by_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<L2BlockBundle>, SyncPeerError>;

    async fn fetch_block_by_id(
        &self,
        block_id: &L2BlockId,
    ) -> Result<Option<L2BlockBundle>, SyncPeerError>;
}

pub struct RpcSyncPeer<RPC: AlpenApiClient + AlpenSyncApiClient + Send + Sync> {
    rpc_client: RPC,
}

impl<RPC: AlpenApiClient + AlpenSyncApiClient + Send + Sync> RpcSyncPeer<RPC> {
    pub fn new(rpc_client: RPC) -> Self {
        Self { rpc_client }
    }
}

#[async_trait::async_trait]
impl<RPC: AlpenApiClient + AlpenSyncApiClient + Send + Sync> SyncPeer for RpcSyncPeer<RPC> {
    async fn fetch_sync_status(&self) -> Result<NodeSyncStatus, SyncPeerError> {
        let status = self
            .rpc_client
            .get_sync_status()
            .await
            .map_err(|e| SyncPeerError::Other(e.to_string()))?;
        Ok(status)
    }

    async fn fetch_blocks_by_height(
        &self,
        height: u64,
    ) -> Result<Vec<L2BlockBundle>, SyncPeerError> {
        let serialized_blocks = self
            .rpc_client
            .sync_blocks_by_height(height)
            .await
            .map_err(|e| SyncPeerError::Other(e.to_string()))?;

        match borsh::from_slice(&serialized_blocks) {
            Ok(blocks) => Ok(blocks),
            Err(err) => {
                error!("failed to deserialize blocks: {err}");
                Err(SyncPeerError::Other(err.to_string()))
            }
        }
    }

    async fn fetch_blocks_by_range(
        &self,
        start_height: u64,
        end_height: u64,
    ) -> Result<Vec<L2BlockBundle>, SyncPeerError> {
        let serialized_blocks = self
            .rpc_client
            .sync_blocks_by_range(start_height, end_height)
            .await
            .map_err(|e| SyncPeerError::Other(e.to_string()))?;

        match borsh::from_slice(&serialized_blocks) {
            Ok(blocks) => Ok(blocks),
            Err(err) => {
                error!("failed to deserialize blocks: {err}");
                Err(SyncPeerError::Other(err.to_string()))
            }
        }
    }

    async fn fetch_block_by_id(
        &self,
        block_id: &L2BlockId,
    ) -> Result<Option<L2BlockBundle>, SyncPeerError> {
        let serialized_block = self
            .rpc_client
            .sync_block_by_id(*block_id)
            .await
            .map_err(|e| SyncPeerError::NetworkError(e.to_string()))?
            .ok_or(SyncPeerError::MissingBlock(*block_id))?;

        match borsh::from_slice(&serialized_block) {
            Ok(blocks) => Ok(blocks),
            Err(err) => {
                error!("failed to deserialize blocks: {err}");
                Err(SyncPeerError::DeserializationError(err.to_string()))
            }
        }
    }
}
