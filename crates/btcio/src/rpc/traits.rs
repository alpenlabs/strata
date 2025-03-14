use async_trait::async_trait;
use bitcoin::{bip32::Xpriv, block::Header, Address, Block, BlockHash, Network, Transaction, Txid};

use crate::rpc::{
    client::ClientResult,
    types::{
        CreateRawTransaction, GetBlockchainInfo, GetRawTransactionVerbosityOne,
        GetRawTransactionVerbosityZero, GetTransaction, GetTxOut, ImportDescriptor,
        ImportDescriptorResult, ListTransactions, ListUnspent, PreviousTransactionOutput,
        SignRawTransactionWithWallet, SubmitPackage, TestMempoolAccept,
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
pub trait ReaderRpc {
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

    /// Gets a [`Header`] with the given hash.
    async fn get_block_header(&self, hash: &BlockHash) -> ClientResult<Header>;

    /// Gets a [`Block`] with the given hash.
    async fn get_block(&self, hash: &BlockHash) -> ClientResult<Block>;

    /// Gets a block height with the given hash.
    async fn get_block_height(&self, hash: &BlockHash) -> ClientResult<u64>;

    /// Gets a [`Header`] at given height.
    async fn get_block_header_at(&self, height: u64) -> ClientResult<Header>;

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

    /// Gets the timestamp in the block header of the current best block in bitcoin.
    ///
    /// # Note
    ///
    /// Time is Unix epoch time in seconds.
    async fn get_current_timestamp(&self) -> ClientResult<u32>;

    /// Gets all transaction ids in mempool.
    async fn get_raw_mempool(&self) -> ClientResult<Vec<Txid>>;

    /// Gets a raw transaction by its [`Txid`].
    async fn get_raw_transaction_verbosity_zero(
        &self,
        txid: &Txid,
    ) -> ClientResult<GetRawTransactionVerbosityZero>;

    /// Gets a raw transaction by its [`Txid`].
    async fn get_raw_transaction_verbosity_one(
        &self,
        txid: &Txid,
    ) -> ClientResult<GetRawTransactionVerbosityOne>;

    /// Returns details about an unspent transaction output.
    async fn get_tx_out(
        &self,
        txid: &Txid,
        vout: u32,
        include_mempool: bool,
    ) -> ClientResult<GetTxOut>;

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
pub trait BroadcasterRpc {
    /// Sends a raw transaction to the network.
    ///
    /// # Parameters
    ///
    /// - `tx`: The raw transaction to send. This should be a byte array containing the serialized
    ///   raw transaction data.
    async fn send_raw_transaction(&self, tx: &Transaction) -> ClientResult<Txid>;

    /// Tests if a raw transaction is valid.
    async fn test_mempool_accept(&self, tx: &Transaction) -> ClientResult<Vec<TestMempoolAccept>>;

    /// Submit a package of raw transactions (serialized, hex-encoded) to local node.
    ///
    /// The package will be validated according to consensus and mempool policy rules. If any
    /// transaction passes, it will be accepted to mempool. This RPC is experimental and the
    /// interface may be unstable. Refer to doc/policy/packages.md for documentation on package
    /// policies.
    ///
    /// # Warning
    ///
    /// Successful submission does not mean the transactions will propagate throughout the network.
    async fn submit_package(&self, txs: &[Transaction]) -> ClientResult<SubmitPackage>;
}

/// Wallet functionality that any Bitcoin client **without private keys** that
/// interacts with the Bitcoin network should provide.
///
/// For signing transactions, see [`SignerRpc`].
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
pub trait WalletRpc {
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

    /// Creates a raw transaction.
    async fn create_raw_transaction(
        &self,
        raw_tx: CreateRawTransaction,
    ) -> ClientResult<Transaction>;
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
pub trait SignerRpc {
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
        prev_outputs: Option<Vec<PreviousTransactionOutput>>,
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

/// Marker trait for [`ReaderRpc`], [`SignerRpc`] and [`WalletRpc`]
#[async_trait]
pub trait WriterRpc: ReaderRpc + SignerRpc + WalletRpc {}

impl<T: ReaderRpc + SignerRpc + WalletRpc> WriterRpc for T {}
