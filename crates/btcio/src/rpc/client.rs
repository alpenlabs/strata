use async_trait::async_trait;
use bitcoin::{Amount, Txid};
use bitcoin::{Block, BlockHash, Transaction};
use bitcoincore_rpc_async::Error as RpcError;
use bitcoincore_rpc_async::{Auth, Client};

use super::traits::BitcoinClient;
use super::types::{RawUTXO, RpcBlockchainInfo, RpcTransactionInfo};

/// Thin wrapper around the [`bitcoincore_rpc_async`]'s [`Client`].
///
/// Provides a simple interface to interact asynchronously with a
/// Bitcoin Core node via JSON-RPC.
#[derive(Debug)]
pub struct BitcoinDClient(Client);

impl BitcoinDClient {
    /// Creates a new [`BitcoinDClient`] instance.
    ///
    /// # Note
    ///
    /// The only supported [`Auth`] method is [`UserPass`](Auth::UserPass),
    /// by providing a `username` and `password`.
    pub async fn new(url: String, username: String, password: String) -> Result<Self, RpcError> {
        let auth = Auth::UserPass(username, password);
        Ok(BitcoinDClient(Client::new(url, auth).await?))
    }
}

#[async_trait]
impl BitcoinClient for BitcoinDClient {
    async fn estimate_smart_fee(&self, conf_target: u16) -> Result<u64, RpcError> {
        self.estimate_smart_fee(conf_target).await
    }

    async fn get_block(&self, hash: BlockHash) -> Result<Block, RpcError> {
        self.get_block(hash).await
    }

    async fn get_block_at(&self, height: u64) -> Result<Block, RpcError> {
        self.get_block_at(height).await
    }

    async fn get_block_count(&self) -> Result<u64, RpcError> {
        self.get_block_count().await
    }

    async fn get_block_hash(&self, height: u64) -> Result<BlockHash, RpcError> {
        self.get_block_hash(height).await
    }

    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, RpcError> {
        self.get_blockchain_info().await
    }

    async fn get_new_address(&self) -> Result<String, RpcError> {
        self.get_new_address().await
    }

    async fn get_raw_mempool(&self) -> Result<Vec<Txid>, RpcError> {
        self.get_raw_mempool().await
    }

    async fn get_transaction(&self, txid: Txid) -> Result<Transaction, RpcError> {
        self.get_transaction(txid).await
    }

    async fn get_transaction_confirmations<T: AsRef<[u8; 32]> + Send>(
        &self,
        txid: T,
    ) -> Result<u64, RpcError> {
        self.get_transaction_confirmations(txid).await
    }

    async fn get_transaction_info(&self, txid: Txid) -> Result<RpcTransactionInfo, RpcError> {
        self.get_transaction_info(txid).await
    }

    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, RpcError> {
        self.get_utxos().await
    }

    async fn list_since_block(
        &self,
        block_hash: BlockHash,
    ) -> Result<Vec<(String, u64)>, RpcError> {
        self.list_since_block(block_hash).await
    }

    async fn list_transactions(&self, count: Option<u32>) -> Result<Vec<(String, u64)>, RpcError> {
        self.list_transactions(count).await
    }

    async fn list_wallets(&self) -> Result<Vec<String>, RpcError> {
        self.list_wallets().await
    }

    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(&self, tx: T) -> Result<Txid, RpcError> {
        self.send_raw_transaction(tx).await
    }

    async fn send_to_address(&self, address: &str, amount: Amount) -> Result<Txid, RpcError> {
        self.send_to_address(address, amount).await
    }

    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: Transaction,
    ) -> Result<Transaction, RpcError> {
        self.sign_raw_transaction_with_wallet(tx).await
    }
}
