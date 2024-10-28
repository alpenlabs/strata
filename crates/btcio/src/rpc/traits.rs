use async_trait::async_trait;
use bitcoin::{bip32::Xpriv, Address, Block, BlockHash, Network, Transaction, Txid};

use crate::rpc::{
    client::ClientResult,
    types::{
        GetBlockchainInfo, GetTransaction, ImportDescriptor, ImportDescriptorResult,
        ListTransactions, ListUnspent, SignRawTransactionWithWallet,
    },
};

/// Basic functionality that any Bitcoin client that interacts with the
/// Bitcoin network should provide.
///
/// # Note
///
/// This is a fully `async` trait. The user should be responsible for
/// handling the `async` nature of the trait methods. And if implementing
/// this trait for a specific type that is not `async`, the user should
/// consider wrapping with [`tokio`](tokio)'s
/// [`spawn_blocking`](tokio::task::spawn_blocking) or any other method.
#[async_trait]
pub trait Reader {
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
    async fn estimate_smart_fee(&self, conf_target: u16) -> ClientResult<u64>;

    /// Gets a [`Block`] with the given hash.
    async fn get_block(&self, hash: &BlockHash) -> ClientResult<Block>;

    /// Gets a [`Block`] at given height.
    async fn get_block_at(&self, height: u64) -> ClientResult<Block>;

    /// Gets the height of the most-work fully-validated chain.
    ///
    /// # Note
    ///
    /// The genesis block has a height of 0.
    async fn get_block_count(&self) -> ClientResult<u64>;

    /// Gets the [`BlockHash`] at given height.
    async fn get_block_hash(&self, height: u64) -> ClientResult<BlockHash>;

    /// Gets various state info regarding blockchain processing.
    async fn get_blockchain_info(&self) -> ClientResult<GetBlockchainInfo>;

    /// Gets all transaction ids in mempool.
    async fn get_raw_mempool(&self) -> ClientResult<Vec<Txid>>;

    /// Gets the underlying [`Network`] information.
    async fn network(&self) -> ClientResult<Network>;
}

/// Broadcasting functionality that any Bitcoin client that interacts with the
/// Bitcoin network should provide.
///
/// # Note
///
/// This is a fully `async` trait. The user should be responsible for
/// handling the `async` nature of the trait methods. And if implementing
/// this trait for a specific type that is not `async`, the user should
/// consider wrapping with [`tokio`](https://tokio.rs)'s
/// [`spawn_blocking`](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
/// or any other method.
#[async_trait]
pub trait Broadcaster {
    /// Sends a raw transaction to the network.
    ///
    /// # Parameters
    ///
    /// - `tx`: The raw transaction to send. This should be a byte array containing the serialized
    ///   raw transaction data.
    async fn send_raw_transaction(&self, tx: &Transaction) -> ClientResult<Txid>;
}

/// Wallet functionality that any Bitcoin client **without private keys** that
/// interacts with the Bitcoin network should provide.
///
/// For signing transactions, see [`Signer`].
///
/// # Note
///
/// This is a fully `async` trait. The user should be responsible for
/// handling the `async` nature of the trait methods. And if implementing
/// this trait for a specific type that is not `async`, the user should
/// consider wrapping with [`tokio`](https://tokio.rs)'s
/// [`spawn_blocking`](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
/// or any other method.
#[async_trait]
pub trait Wallet {
    /// Generates new address under own control for the underlying Bitcoin
    /// client's wallet.
    async fn get_new_address(&self) -> ClientResult<Address>;

    /// Gets information related to a transaction.
    ///
    /// # Note
    ///
    /// This assumes that the transaction is present in the underlying Bitcoin
    /// client's wallet.
    async fn get_transaction(&self, txid: &Txid) -> ClientResult<GetTransaction>;

    /// Gets all Unspent Transaction Outputs (UTXOs) for the underlying Bitcoin
    /// client's wallet.
    async fn get_utxos(&self) -> ClientResult<Vec<ListUnspent>>;

    /// Lists transactions in the underlying Bitcoin client's wallet.
    ///
    /// # Parameters
    ///
    /// - `count`: The number of transactions to list. If `None`, assumes a maximum of 10
    ///   transactions.
    async fn list_transactions(&self, count: Option<usize>) -> ClientResult<Vec<ListTransactions>>;

    /// Lists all wallets in the underlying Bitcoin client.
    async fn list_wallets(&self) -> ClientResult<Vec<String>>;
}

/// Signing functionality that any Bitcoin client **with private keys** that
/// interacts with the Bitcoin network should provide.
///
/// # Note
///
/// This is a fully `async` trait. The user should be responsible for
/// handling the `async` nature of the trait methods. And if implementing
/// this trait for a specific type that is not `async`, the user should
/// consider wrapping with [`tokio`](https://tokio.rs)'s
/// [`spawn_blocking`](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)
/// or any other method.
#[async_trait]
pub trait Signer {
    /// Signs a transaction using the keys available in the underlying Bitcoin
    /// client's wallet and returns a signed transaction.
    ///
    /// # Note
    ///
    /// The returned signed transaction might not be consensus-valid if it
    /// requires additional signatures, such as in a multisignature context.
    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: &Transaction,
    ) -> ClientResult<SignRawTransactionWithWallet>;

    /// Gets the underlying [`Xpriv`] from the wallet.
    async fn get_xpriv(&self) -> ClientResult<Option<Xpriv>>;

    /// Imports the descriptors into the wallet.
    async fn import_descriptors(
        &self,
        descriptors: Vec<ImportDescriptor>,
        wallet_name: String,
    ) -> ClientResult<Vec<ImportDescriptorResult>>;
}
