use async_trait::async_trait;
use bitcoin::{Block, BlockHash, Network, Transaction, Txid};

use super::{
    types::{RawUTXO, RpcBlockchainInfo},
    ClientError,
};

/// Basic functionality that any Bitcoin client that interacts with the
/// Bitcoin network should provide.
#[async_trait]
pub trait BitcoinClient: Sync + Send + 'static {
    /// Estimates the approximate fee per kilobyte needed for a transaction
    /// to begin confirmation within conf_target blocks if possible and return
    /// the number of blocks for which the estimate is valid.
    ///
    /// # Note
    ///
    /// Uses virtual transaction size as defined in
    /// [BIP 141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
    /// (witness data is discounted).
    ///
    /// By default uses the estimate mode of `CONSERVATIVE` which is the
    /// default in Bitcoin Core v27.
    async fn estimate_smart_fee(&self) -> Result<u64, ClientError>;

    /// Gets a [`Block`] at given height.
    async fn get_block_at(&self, height: u64) -> Result<Block, ClientError>;

    /// Gets the [`BlockHash`] at given height.
    async fn get_block_hash(&self, height: u64) -> Result<BlockHash, ClientError>;

    /// Gets various state info regarding blockchain processing.
    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, ClientError>;

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
    ) -> Result<u64, ClientError>;

    /// Gets all Unspent Transaction Outputs (UTXOs) for the underlying Bitcoin
    /// client's wallet.
    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, ClientError>;

    /// Sends a raw transaction to the network.
    ///
    /// # Parameters
    ///
    /// - `tx`: The raw transaction to send. This should be a byte array containing the serialized
    ///   raw transaction data.
    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(&self, tx: T)
        -> Result<Txid, ClientError>;

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
    ) -> Result<Transaction, ClientError>;

    /// Returns the [`Network`] of the underlying Bitcoin client.
    fn get_network(&self) -> Network;
}
