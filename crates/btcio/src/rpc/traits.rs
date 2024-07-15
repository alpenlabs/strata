use async_trait::async_trait;
use bitcoin::{Block, BlockHash, Network, Txid};

use super::{
    types::{RawUTXO, RpcBlockchainInfo},
    ClientError,
};

#[async_trait]
pub trait L1Client: Sync + Send + 'static {
    /// Corresponds to `getblockchaininfo`.
    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, ClientError>;

    /// Fetches the block at given height
    async fn get_block_at(&self, height: u64) -> Result<Block, ClientError>;

    /// Fetches the block hash at given height
    async fn get_block_hash(&self, height: u64) -> Result<BlockHash, ClientError>;

    /// Sends a raw transaction to the network
    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(&self, tx: T)
        -> Result<Txid, ClientError>;

    /// get number of confirmations for txid
    /// 0 confirmations means tx is still in mempool
    async fn get_transaction_confirmations<T: AsRef<[u8]> + Send>(
        &self,
        txid: T,
    ) -> Result<u64, ClientError>;
    //
    // TODO: add others as necessary
}

#[async_trait]
pub trait SeqL1Client: Sync + Send + 'static {
    /// Get utxos
    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, ClientError>;

    /// estimate_smart_fee estimates the fee to confirm a transaction in the next block
    async fn estimate_smart_fee(&self) -> Result<u64, ClientError>;

    /// Network of the rpc client
    fn network(&self) -> Network;
}
