use async_trait::async_trait;
use bitcoin::{Amount, Block, BlockHash, Transaction, Txid};
use bitcoincore_rpc_async::Error as RpcError;

use super::types::{RawUTXO, RpcBlockchainInfo};

/// Basic functionality that any Bitcoin client that interacts with the
/// Bitcoin network should provide.
#[async_trait]
pub trait BitcoinClient: Sync + Send + 'static {
    /// Estimates the approximate fee per kilobyte needed for a transaction
    /// to begin confirmation within conf_target blocks if possible and return
    /// the number of blocks for which the estimate is valid.
    ///
    /// # Parameters
    ///
    /// - `conf_target`: Confirmation target in blocks.
    ///
    /// # Note
    ///
    /// Uses virtual transaction size as defined in
    /// [BIP 141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
    /// (witness data is discounted).
    ///
    /// By default uses the estimate mode of `CONSERVATIVE` which is the
    /// default in Bitcoin Core v27.
    async fn estimate_smart_fee(&self, conf_target: u16) -> Result<u64, RpcError>;

    /// Gets a [`Block`] with the given hash.
    async fn get_block(&self, hash: BlockHash) -> Result<Block, RpcError>;

    /// Gets a [`Block`] at given height.
    async fn get_block_at(&self, height: u64) -> Result<Block, RpcError>;

    /// Gets the height of the most-work fully-validated chain.
    ///
    /// # Note
    ///
    /// The genesis block has a height of 0.
    async fn get_block_count(&self) -> Result<u64, RpcError>;

    /// Gets the [`BlockHash`] at given height.
    async fn get_block_hash(&self, height: u64) -> Result<BlockHash, RpcError>;

    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, RpcError>;
    /// Gets various state info regarding blockchain processing.

    /// Generates new address under own control for the underlying Bitcoin
    /// client's wallet.
    async fn get_new_address(&self) -> Result<String, RpcError>;

    /// Gets all transaction ids in mempool.
    async fn get_raw_mempool(&self) -> Result<Vec<Txid>, RpcError>;

    /// Gets a transaction with a given transaction id (txid).
    async fn get_transaction(&self, txid: Txid) -> Result<Transaction, RpcError>;

    /// Gets the number of confirmations for a given transaction id
    /// (txid).
    ///
    /// # Parameters
    ///
    /// - `txid`: The transaction id to fetch confirmations for. This should be a 32-byte array
    ///   containing the transaction id.
    ///
    /// # Note
    ///
    /// If a transaction has 0 confirmations, it means that the transaction
    /// is still in the mempool, i. e. it has not been included in a block yet.
    async fn get_transaction_confirmations<T: AsRef<[u8; 32]> + Send>(
        &self,
        txid: T,
    ) -> Result<u64, RpcError>;

    /// Gets all Unspent Transaction Outputs (UTXOs) for the underlying Bitcoin
    /// client's wallet.
    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, RpcError>;

    /// Gets all transactions in blocks since block [`Blockhash`].
    async fn list_since_block(&self, block_hash: BlockHash)
        -> Result<Vec<(String, u64)>, RpcError>;

    /// Lists transactions in the underlying Bitcoin client's wallet.
    ///
    /// # Parameters
    ///
    /// - `count`: The number of transactions to list. If `None`, assumes a maximum of 10
    ///   transactions.
    async fn list_transactions(&self, count: Option<u32>) -> Result<Vec<(String, u64)>, RpcError>;

    /// Lists all wallets in the underlying Bitcoin client.
    async fn list_wallets(&self) -> Result<Vec<String>, RpcError>;

    /// Sends a raw transaction to the network.
    ///
    /// # Parameters
    ///
    /// - `tx`: The raw transaction to send. This should be a byte array containing the serialized
    ///   raw transaction data.
    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(&self, tx: T) -> Result<Txid, RpcError>;

    /// Sends an amount to a given address.
    async fn send_to_address(&self, address: &str, amount: Amount) -> Result<Txid, RpcError>;

    /// Signs a transaction using the keys available in the underlying Bitcoin
    /// client's wallet and returns a signed transaction.
    ///
    /// # Note
    ///
    /// The returned signed transaction might not be consensus-valid if it
    /// requires additional signatures, such as in a multisignature context.
    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: Transaction,
    ) -> Result<Transaction, RpcError>;
}
