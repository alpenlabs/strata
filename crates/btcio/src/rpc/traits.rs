use async_trait::async_trait;
use bitcoin::{Block, BlockHash};

use super::types::RpcBlockchainInfo;

#[async_trait]
pub trait L1Client: Sync + Send + 'static {
    /// Corresponds to `getblockchaininfo`.
    async fn get_blockchain_info(&self) -> anyhow::Result<RpcBlockchainInfo>;

    /// Fetches the block at given height
    async fn get_block_at(&self, height: u64) -> anyhow::Result<Block>;

    /// Fetches the block hash at given height
    async fn get_block_hash(&self, height: u64) -> anyhow::Result<BlockHash>;

    // TODO: add others as necessary
}
