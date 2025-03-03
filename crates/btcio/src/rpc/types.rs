use std::collections::BTreeMap;

use bitcoin::{
    absolute::Height,
    address::{self, NetworkUnchecked},
    block::Header,
    consensus::{self, encode},
    Address, Amount, Block, BlockHash, SignedAmount, Transaction, Txid, Wtxid,
};
use serde::{
    de::{self, IntoDeserializer, Visitor},
    Deserialize, Deserializer, Serialize,
};
use tracing::*;

use crate::rpc::error::SignRawTransactionWithWalletError;

/// The category of a transaction.
///
/// This is one of the results of `listtransactions` RPC method.
///
/// # Note
///
/// This is a subset of the categories available in Bitcoin Core.
/// It also assumes that the transactions are present in the underlying Bitcoin
/// client's wallet.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionCategory {
    /// Transactions sent.
    Send,
    /// Non-coinbase transactions received.
    Receive,
    /// Coinbase transactions received with more than 100 confirmations.
    Generate,
    /// Coinbase transactions received with 100 or less confirmations.
    Immature,
    /// Orphaned coinbase transactions received.
    Orphan,
}

/// Result of JSON-RPC method `getblockchaininfo`.
///
/// Method call: `getblockchaininfo`
///
/// > Returns an object containing various state info regarding blockchain processing.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GetBlockchainInfo {
    /// Current network name as defined in BIP70 (main, test, signet, regtest).
    pub chain: String,
    /// The current number of blocks processed in the server.
    pub blocks: u64,
    /// The current number of headers we have validated.
    pub headers: u64,
    /// The hash of the currently best block.
    #[serde(rename = "bestblockhash")]
    pub best_block_hash: String,
    /// The current difficulty.
    pub difficulty: f64,
    /// Median time for the current best block.
    #[serde(rename = "mediantime")]
    pub median_time: u64,
    /// Estimate of verification progress (between 0 and 1).
    #[serde(rename = "verificationprogress")]
    pub verification_progress: f64,
    /// Estimate of whether this node is in Initial Block Download (IBD) mode.
    #[serde(rename = "initialblockdownload")]
    pub initial_block_download: bool,
    /// Total amount of work in active chain, in hexadecimal.
    #[serde(rename = "chainwork")]
    pub chain_work: String,
    /// The estimated size of the block and undo files on disk.
    pub size_on_disk: u64,
    /// If the blocks are subject to pruning.
    pub pruned: bool,
    /// Lowest-height complete block stored (only present if pruning is enabled).
    #[serde(rename = "pruneheight")]
    pub prune_height: Option<u64>,
    /// Whether automatic pruning is enabled (only present if pruning is enabled).
    pub automatic_pruning: Option<bool>,
    /// The target size used by pruning (only present if automatic pruning is enabled).
    pub prune_target_size: Option<u64>,
}

/// Result of JSON-RPC method `getblockheader` with verbosity set to 0.
///
/// A string that is serialized, hex-encoded data for block 'hash'.
///
/// Method call: `getblockheader "blockhash" ( verbosity )`
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GetBlockHeaderVerbosityZero(pub String);

impl GetBlockHeaderVerbosityZero {
    /// Converts json straight to a [`Header`].
    pub fn header(self) -> Result<Header, encode::FromHexError> {
        let header: Header = encode::deserialize_hex(&self.0)?;
        Ok(header)
    }
}

/// Result of JSON-RPC method `getblock` with verbosity set to 0.
///
/// A string that is serialized, hex-encoded data for block 'hash'.
///
/// Method call: `getblock "blockhash" ( verbosity )`
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GetBlockVerbosityZero(pub String);

impl GetBlockVerbosityZero {
    /// Converts json straight to a [`Block`].
    pub fn block(self) -> Result<Block, encode::FromHexError> {
        let block: Block = encode::deserialize_hex(&self.0)?;
        Ok(block)
    }
}

/// Result of JSON-RPC method `getblock` with verbosity set to 1.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GetBlockVerbosityOne {
    /// The block hash (same as provided) in RPC call.
    pub hash: String,
    /// The number of confirmations, or -1 if the block is not on the main chain.
    pub confirmations: i32,
    /// The block size.
    pub size: usize,
    /// The block size excluding witness data.
    #[serde(rename = "strippedsize")]
    pub stripped_size: Option<usize>,
    /// The block weight as defined in BIP-141.
    pub weight: u64,
    /// The block height or index.
    pub height: usize,
    /// The block version.
    pub version: i32,
    /// The block version formatted in hexadecimal.
    #[serde(rename = "versionHex")]
    pub version_hex: String,
    /// The merkle root
    #[serde(rename = "merkleroot")]
    pub merkle_root: String,
    /// The transaction ids
    pub tx: Vec<String>,
    /// The block time expressed in UNIX epoch time.
    pub time: usize,
    /// The median block time expressed in UNIX epoch time.
    #[serde(rename = "mediantime")]
    pub median_time: Option<usize>,
    /// The nonce
    pub nonce: u32,
    /// The bits.
    pub bits: String,
    /// The difficulty.
    pub difficulty: f64,
    /// Expected number of hashes required to produce the chain up to this block (in hex).
    #[serde(rename = "chainwork")]
    pub chain_work: String,
    /// The number of transactions in the block.
    #[serde(rename = "nTx")]
    pub n_tx: u32,
    /// The hash of the previous block (if available).
    #[serde(rename = "previousblockhash")]
    pub previous_block_hash: Option<String>,
    /// The hash of the next block (if available).
    #[serde(rename = "nextblockhash")]
    pub next_block_hash: Option<String>,
}

/// Result of JSON-RPC method `getrawtransaction` with verbosity set to 0.
///
/// A string that is serialized, hex-encoded data for transaction.
///
/// Method call: `getrawtransaction "txid" ( verbosity )`
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GetRawTransactionVerbosityZero(pub String);

impl GetRawTransactionVerbosityZero {
    /// Converts json straight to a [`Transaction`].
    pub fn transaction(self) -> Result<Transaction, encode::FromHexError> {
        let transaction: Transaction = encode::deserialize_hex(&self.0)?;
        Ok(transaction)
    }
}

/// Result of JSON-RPC method `getrawtransaction` with verbosity set to 1.
///
/// Method call: `getrawtransaction "txid" ( verbosity )`
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub struct GetRawTransactionVerbosityOne {
    pub in_active_chain: Option<bool>,
    #[serde(deserialize_with = "deserialize_tx")]
    #[serde(rename = "hex")]
    pub transaction: Transaction,
    pub txid: Txid,
    pub hash: Wtxid,
    pub size: usize,
    pub vsize: usize,
    pub version: u32,
    pub locktime: u32,
    pub blockhash: Option<BlockHash>,
    pub confirmations: Option<u32>,
    pub time: Option<usize>,
    pub blocktime: Option<usize>,
}

/// Result of JSON-RPC method `gettxout`.
///
/// > gettxout "txid" n ( include_mempool )
/// >
/// > Returns details about an unspent transaction output.
/// >
/// > Arguments:
/// > 1. txid               (string, required) The transaction id
/// > 2. n                  (numeric, required) vout number
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GetTxOut {
    /// The hash of the block at the tip of the chain.
    #[serde(rename = "bestblock")]
    pub best_block: String,
    /// The number of confirmations.
    pub confirmations: u32, // TODO: Change this to an i64.
    /// The transaction value in BTC.
    pub value: f64,
    /// The script pubkey.
    #[serde(rename = "scriptPubkey")]
    pub script_pubkey: Option<ScriptPubkey>,
    /// Coinbase or not.
    pub coinbase: bool,
}

/// A script pubkey.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ScriptPubkey {
    /// Script assembly.
    pub asm: String,
    /// Script hex.
    pub hex: String,
    #[serde(rename = "reqSigs")]
    pub req_sigs: i64,
    /// The type, eg pubkeyhash.
    #[serde(rename = "type")]
    pub type_: String,
    /// Bitcoin address.
    pub address: Option<String>,
}

/// Models the arguments of JSON-RPC method `createrawtransaction`.
///
/// # Note
///
/// Assumes that the transaction is always "replaceable" by default and has a locktime of 0.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CreateRawTransaction {
    pub inputs: Vec<CreateRawTransactionInput>,
    pub outputs: Vec<CreateRawTransactionOutput>,
}

/// Models the input of JSON-RPC method `createrawtransaction`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateRawTransactionInput {
    pub txid: String,
    pub vout: u32,
}

/// Models the output of JSON-RPC method `createrawtransaction`.
///
/// The outputs specified as key-value pairs, where the keys is an address,
/// and the values are the amounts to be sent to that address.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum CreateRawTransactionOutput {
    /// A pair of an [`Address`] string and an [`Amount`] in BTC.
    AddressAmount {
        /// An [`Address`] string.
        address: String,
        /// An [`Amount`] in BTC.
        amount: f64,
    },
    /// A payload such as in `OP_RETURN` transactions.
    Data {
        /// The payload.
        data: String,
    },
}

impl Serialize for CreateRawTransactionOutput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            CreateRawTransactionOutput::AddressAmount { address, amount } => {
                let mut map = serde_json::Map::new();
                map.insert(
                    address.clone(),
                    serde_json::Value::Number(serde_json::Number::from_f64(*amount).unwrap()),
                );
                map.serialize(serializer)
            }
            CreateRawTransactionOutput::Data { data } => {
                let mut map = serde_json::Map::new();
                map.insert("data".to_string(), serde_json::Value::String(data.clone()));
                map.serialize(serializer)
            }
        }
    }
}

/// Result of JSON-RPC method `submitpackage`.
///
/// > submitpackage ["rawtx",...] ( maxfeerate maxburnamount )
/// >
/// > Submit a package of raw transactions (serialized, hex-encoded) to local node.
/// > The package will be validated according to consensus and mempool policy rules. If any
/// > transaction passes, it will be accepted to mempool.
/// > This RPC is experimental and the interface may be unstable. Refer to doc/policy/packages.md
/// > for documentation on package policies.
/// > Warning: successful submission does not mean the transactions will propagate throughout the
/// > network.
/// >
/// > Arguments:
/// > 1. package          (json array, required) An array of raw transactions.
/// > The package must solely consist of a child and its parents. None of the parents may depend on
/// > each other.
/// > The package must be topologically sorted, with the child being the last element in the array.
/// > [
/// > "rawtx",     (string)
/// > ...
/// > ]
#[allow(clippy::doc_lazy_continuation)]
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SubmitPackage {
    /// The transaction package result message.
    ///
    /// "success" indicates all transactions were accepted into or are already in the mempool.
    pub package_msg: String,
    /// Transaction results keyed by wtxid.
    #[serde(rename = "tx-results")]
    pub tx_results: BTreeMap<String, SubmitPackageTxResult>,
    /// List of txids of replaced transactions.
    #[serde(rename = "replaced-transactions")]
    pub replaced_transactions: Vec<String>,
}

/// Models the per-transaction result included in the JSON-RPC method `submitpackage`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SubmitPackageTxResult {
    /// The transaction id.
    pub txid: String,
    /// The wtxid of a different transaction with the same txid but different witness found in the
    /// mempool.
    ///
    /// If set, this means the submitted transaction was ignored.
    #[serde(rename = "other-wtxid")]
    pub other_wtxid: Option<String>,
    /// Sigops-adjusted virtual transaction size.
    pub vsize: i64,
    /// Transaction fees.
    pub fees: Option<SubmitPackageTxResultFees>,
    /// The transaction error string, if it was rejected by the mempool
    pub error: Option<String>,
}

/// Models the fees included in the per-transaction result of the JSON-RPC method `submitpackage`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SubmitPackageTxResultFees {
    /// Transaction fee.
    #[serde(rename = "base")]
    pub base_fee: f64,
    /// The effective feerate.
    ///
    /// Will be `None` if the transaction was already in the mempool. For example, the package
    /// feerate and/or feerate with modified fees from the `prioritisetransaction` JSON-RPC method.
    #[serde(rename = "effective-feerate")]
    pub effective_fee_rate: Option<f64>,
    /// If [`Self::effective_fee_rate`] is provided, this holds the wtxid's of the transactions
    /// whose fees and vsizes are included in effective-feerate.
    #[serde(rename = "effective-includes")]
    pub effective_includes: Option<Vec<String>>,
}

/// Result of JSON-RPC method `gettxout`.
///
/// # Note
///
/// This assumes that the UTXOs are present in the underlying Bitcoin
/// client's wallet.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GetTransactionDetail {
    pub address: String,
    pub category: GetTransactionDetailCategory,
    pub amount: f64,
    pub label: Option<String>,
    pub vout: u32,
    pub fee: Option<f64>,
    pub abandoned: Option<bool>,
}

/// Enum to represent the category of a transaction.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GetTransactionDetailCategory {
    Send,
    Receive,
    Generate,
    Immature,
    Orphan,
}

/// Result of the JSON-RPC method `getnewaddress`.
///
/// # Note
///
/// This assumes that the UTXOs are present in the underlying Bitcoin
/// client's wallet.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct GetNewAddress(pub String);

impl GetNewAddress {
    /// Converts json straight to a [`Address`].
    pub fn address(self) -> Result<Address<NetworkUnchecked>, address::ParseError> {
        let address = self.0.parse::<Address<_>>()?;
        Ok(address)
    }
}

/// Models the result of JSON-RPC method `listunspent`.
///
/// # Note
///
/// This assumes that the UTXOs are present in the underlying Bitcoin
/// client's wallet.
///
/// Careful with the amount field. It is a [`SignedAmount`], hence can be negative.
/// Negative amounts for the [`TransactionCategory::Send`], and is positive
/// for all other categories.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct GetTransaction {
    /// The signed amount in BTC.
    #[serde(deserialize_with = "deserialize_signed_bitcoin")]
    pub amount: SignedAmount,
    /// The signed fee in BTC.
    pub confirmations: u64,
    pub generated: Option<bool>,
    pub trusted: Option<bool>,
    pub blockhash: Option<String>,
    pub blockheight: Option<u64>,
    pub blockindex: Option<u32>,
    pub blocktime: Option<u64>,
    /// The transaction id.
    #[serde(deserialize_with = "deserialize_txid")]
    pub txid: Txid,
    pub wtxid: String,
    pub walletconflicts: Vec<String>,
    pub replaced_by_txid: Option<String>,
    pub replaces_txid: Option<String>,
    pub comment: Option<String>,
    pub to: Option<String>,
    pub time: u64,
    pub timereceived: u64,
    #[serde(rename = "bip125-replaceable")]
    pub bip125_replaceable: String,
    pub details: Vec<GetTransactionDetail>,
    /// The transaction itself.
    #[serde(deserialize_with = "deserialize_tx")]
    pub hex: Transaction,
}

impl GetTransaction {
    pub fn block_height(&self) -> u64 {
        if self.confirmations == 0 {
            return 0;
        }
        self.blockheight.unwrap_or_else(|| {
            warn!("Txn confirmed but did not obtain blockheight. Setting height to zero");
            0
        })
    }
}

/// Models the result of JSON-RPC method `listunspent`.
///
/// # Note
///
/// This assumes that the UTXOs are present in the underlying Bitcoin
/// client's wallet.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListUnspent {
    /// The transaction id.
    #[serde(deserialize_with = "deserialize_txid")]
    pub txid: Txid,
    /// The vout value.
    pub vout: u32,
    /// The Bitcoin address.
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Address<NetworkUnchecked>,
    // The associated label, if any.
    pub label: Option<String>,
    /// The script pubkey.
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    /// The transaction output amount in BTC.
    #[serde(deserialize_with = "deserialize_bitcoin")]
    pub amount: Amount,
    /// The number of confirmations.
    pub confirmations: u32,
    /// Whether we have the private keys to spend this output.
    pub spendable: bool,
    /// Whether we know how to spend this output, ignoring the lack of keys.
    pub solvable: bool,
    /// Whether this output is considered safe to spend.
    /// Unconfirmed transactions from outside keys and unconfirmed replacement
    /// transactions are considered unsafe and are not eligible for spending by
    /// `fundrawtransaction` and `sendtoaddress`.
    pub safe: bool,
}

/// Models the result of JSON-RPC method `listtransactions`.
///
/// # Note
///
/// This assumes that the transactions are present in the underlying Bitcoin
/// client's wallet.
///
/// Careful with the amount field. It is a [`SignedAmount`], hence can be negative.
/// Negative amounts for the [`TransactionCategory::Send`], and is positive
/// for all other categories.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ListTransactions {
    /// The Bitcoin address.
    #[serde(deserialize_with = "deserialize_address")]
    pub address: Address<NetworkUnchecked>,
    /// Category of the transaction.
    category: TransactionCategory,
    /// The signed amount in BTC.
    #[serde(deserialize_with = "deserialize_signed_bitcoin")]
    pub amount: SignedAmount,
    /// The label associated with the address, if any.
    pub label: Option<String>,
    /// The number of confirmations.
    pub confirmations: u32,
    pub trusted: Option<bool>,
    pub generated: Option<bool>,
    pub blockhash: Option<String>,
    pub blockheight: Option<u64>,
    pub blockindex: Option<u32>,
    pub blocktime: Option<u64>,
    /// The transaction id.
    #[serde(deserialize_with = "deserialize_txid")]
    pub txid: Txid,
}

/// Models the result of JSON-RPC method `testmempoolaccept`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct TestMempoolAccept {
    /// The transaction id.
    #[serde(deserialize_with = "deserialize_txid")]
    pub txid: Txid,
    /// Rejection reason, if any.
    pub reject_reason: Option<String>,
}

/// Models the result of JSON-RPC method `signrawtransactionwithwallet`.
///
/// # Note
///
/// This assumes that the transactions are present in the underlying Bitcoin
/// client's wallet.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SignRawTransactionWithWallet {
    /// The Transaction ID.
    pub hex: String,
    /// If the transaction has a complete set of signatures.
    pub complete: bool,
    /// Errors, if any.
    pub errors: Option<Vec<SignRawTransactionWithWalletError>>,
}

/// Models the optional previous transaction outputs argument for the method
/// `signrawtransactionwithwallet`.
///
/// These are the outputs that this transaction depends on but may not yet be in the block chain.
/// Widely used for One Parent One Child (1P1C) Relay in Bitcoin >28.0.
///
/// > transaction outputs
/// > [
/// > {                            (json object)
/// > "txid": "hex",             (string, required) The transaction id
/// > "vout": n,                 (numeric, required) The output number
/// > "scriptPubKey": "hex",     (string, required) The output script
/// > "redeemScript": "hex",     (string, optional) (required for P2SH) redeem script
/// > "witnessScript": "hex",    (string, optional) (required for P2WSH or P2SH-P2WSH) witness
/// > script
/// > "amount": amount,          (numeric or string, optional) (required for Segwit inputs) the
/// > amount spent
/// > },
/// > ...
/// > ]
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct PreviousTransactionOutput {
    /// The transaction id.
    #[serde(deserialize_with = "deserialize_txid")]
    pub txid: Txid,
    /// The output number.
    pub vout: u32,
    /// The output script.
    #[serde(rename = "scriptPubKey")]
    pub script_pubkey: String,
    /// The redeem script.
    #[serde(rename = "redeemScript")]
    pub redeem_script: Option<String>,
    /// The witness script.
    #[serde(rename = "witnessScript")]
    pub witness_script: Option<String>,
    /// The amount spent.
    pub amount: Option<f64>,
}

/// Models the result of the JSON-RPC method `listdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListDescriptors {
    /// The descriptors
    pub descriptors: Vec<ListDescriptor>,
}

/// Models the Descriptor in the result of the JSON-RPC method `listdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListDescriptor {
    /// The descriptor.
    pub desc: String,
}

/// Models the result of the JSON-RPC method `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ImportDescriptors {
    /// The descriptors
    pub descriptors: Vec<ListDescriptor>,
}

/// Models the Descriptor in the result of the JSON-RPC method `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ImportDescriptor {
    /// The descriptor.
    pub desc: String,
    /// Set this descriptor to be the active descriptor
    /// for the corresponding output type/externality.
    pub active: Option<bool>,
    /// Time from which to start rescanning the blockchain for this descriptor,
    /// in UNIX epoch time. Can also be a string "now"
    pub timestamp: String,
}
/// Models the Descriptor in the result of the JSON-RPC method `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ImportDescriptorResult {
    /// Result.
    pub success: bool,
}

/// Models the `createwallet` JSON-RPC method.
///
/// # Note
///
/// This can also be used for the `loadwallet` JSON-RPC method.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CreateWallet {
    /// Wallet name
    pub wallet_name: String,
    /// Load on startup
    pub load_on_startup: Option<bool>,
}

/// Deserializes the amount in BTC into proper [`Amount`]s.
fn deserialize_bitcoin<'d, D>(deserializer: D) -> Result<Amount, D::Error>
where
    D: Deserializer<'d>,
{
    struct SatVisitor;

    impl Visitor<'_> for SatVisitor {
        type Value = Amount;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a float representation of btc values expected")
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let amount = Amount::from_btc(v).expect("Amount deserialization failed");
            Ok(amount)
        }
    }
    deserializer.deserialize_any(SatVisitor)
}

/// Deserializes the *signed* amount in BTC into proper [`SignedAmount`]s.
fn deserialize_signed_bitcoin<'d, D>(deserializer: D) -> Result<SignedAmount, D::Error>
where
    D: Deserializer<'d>,
{
    struct SatVisitor;

    impl Visitor<'_> for SatVisitor {
        type Value = SignedAmount;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a float representation of btc values expected")
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let signed_amount = SignedAmount::from_btc(v).expect("Amount deserialization failed");
            Ok(signed_amount)
        }
    }
    deserializer.deserialize_any(SatVisitor)
}

fn deserialize_signed_bitcoin_option<'d, D>(
    deserializer: D,
) -> Result<Option<SignedAmount>, D::Error>
where
    D: Deserializer<'d>,
{
    let f: Option<f64> = Option::deserialize(deserializer)?;
    match f {
        Some(v) => deserialize_signed_bitcoin(v.into_deserializer()).map(Some),
        None => Ok(None),
    }
}

/// Deserializes the transaction id string into proper [`Txid`]s.
fn deserialize_txid<'d, D>(deserializer: D) -> Result<Txid, D::Error>
where
    D: Deserializer<'d>,
{
    struct TxidVisitor;

    impl Visitor<'_> for TxidVisitor {
        type Value = Txid;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a transaction id string expected")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let txid = v.parse::<Txid>().expect("invalid txid");

            Ok(txid)
        }
    }
    deserializer.deserialize_any(TxidVisitor)
}

/// Deserializes the transaction hex string into proper [`Transaction`]s.
fn deserialize_tx<'d, D>(deserializer: D) -> Result<Transaction, D::Error>
where
    D: Deserializer<'d>,
{
    struct TxVisitor;

    impl Visitor<'_> for TxVisitor {
        type Value = Transaction;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a transaction hex string expected")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let tx = consensus::encode::deserialize_hex::<Transaction>(v)
                .expect("failed to deserialize tx hex");
            Ok(tx)
        }
    }
    deserializer.deserialize_any(TxVisitor)
}

/// Deserializes the address string into proper [`Address`]s.
///
/// # Note
///
/// The user is responsible for ensuring that the address is valid,
/// since this functions returns an [`Address<NetworkUnchecked>`].
fn deserialize_address<'d, D>(deserializer: D) -> Result<Address<NetworkUnchecked>, D::Error>
where
    D: Deserializer<'d>,
{
    struct AddressVisitor;
    impl Visitor<'_> for AddressVisitor {
        type Value = Address<NetworkUnchecked>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a Bitcoin address string expected")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let address = v
                .parse::<Address<_>>()
                .expect("Address deserialization failed");
            Ok(address)
        }
    }
    deserializer.deserialize_any(AddressVisitor)
}

/// Deserializes the blockhash string into proper [`BlockHash`]s.
fn deserialize_blockhash<'d, D>(deserializer: D) -> Result<BlockHash, D::Error>
where
    D: Deserializer<'d>,
{
    struct BlockHashVisitor;

    impl Visitor<'_> for BlockHashVisitor {
        type Value = BlockHash;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a blockhash string expected")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let blockhash = consensus::encode::deserialize_hex::<BlockHash>(v)
                .expect("BlockHash deserialization failed");
            Ok(blockhash)
        }
    }
    deserializer.deserialize_any(BlockHashVisitor)
}

/// Deserializes the height string into proper [`Height`]s.
fn deserialize_height<'d, D>(deserializer: D) -> Result<Height, D::Error>
where
    D: Deserializer<'d>,
{
    struct HeightVisitor;

    impl Visitor<'_> for HeightVisitor {
        type Value = Height;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a height u32 string expected")
        }

        fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let height = Height::from_consensus(v).expect("Height deserialization failed");
            Ok(height)
        }
    }
    deserializer.deserialize_any(HeightVisitor)
}
