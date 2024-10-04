use bitcoin::{
    absolute::Height, address::NetworkUnchecked, consensus, Address, Amount, BlockHash,
    SignedAmount, Transaction, Txid,
};
use bitcoind_json_rpc_types::v26::GetTransactionDetail;
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
///
/// We can upstream this to [`bitcoind_json_rpc_types`].
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
///
/// We can upstream this to [`bitcoind_json_rpc_types`].
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
///
/// We can upstream this to [`bitcoind_json_rpc_types`].
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

/// Models the result of JSON-RPC method `signrawtransactionwithwallet`.
///
/// # Note
///
/// This assumes that the transactions are present in the underlying Bitcoin
/// client's wallet.
///
/// We can upstream this to [`bitcoind_json_rpc_types`].
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SignRawTransactionWithWallet {
    /// The Transaction ID.
    pub hex: String,
    /// If the transaction has a complete set of signatures.
    pub complete: bool,
    /// Errors, if any.
    pub errors: Option<Vec<SignRawTransactionWithWalletError>>,
}

/// Models the result of the JSON-RPC method `listdescriptors`
/// or the argument to `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListDescriptors {
    /// The descriptors
    pub descriptors: Vec<ListDescriptor>,
}

/// Models the Descriptor in the result of the JSON-RPC method `listdescriptors`
/// or the argument to `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ListDescriptor {
    /// The descriptor.
    pub desc: String,
    /// Set this descriptor to be the active descriptor
    /// for the corresponding output type/externality.
    pub active: Option<bool>,
    /// Time from which to start rescanning the blockchain for this descriptor,
    /// in UNIX epoch time. Can also be a string "now"
    pub timestamp: String,
}

/// Models the result of the JSON-RPC method `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ImportDescriptors {
    /// The descriptors
    pub descriptors: Vec<ImportDescriptor>,
}

/// Models the Descriptor in the result of the JSON-RPC method `importdescriptors`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ImportDescriptor {
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

    impl<'d> Visitor<'d> for SatVisitor {
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

    impl<'d> Visitor<'d> for SatVisitor {
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

    impl<'d> Visitor<'d> for TxidVisitor {
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

    impl<'d> Visitor<'d> for TxVisitor {
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
    impl<'d> Visitor<'d> for AddressVisitor {
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

    impl<'d> Visitor<'d> for BlockHashVisitor {
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

    impl<'d> Visitor<'d> for HeightVisitor {
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
