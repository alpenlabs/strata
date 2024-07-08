use bitcoin::BlockHash;
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize)]

pub struct GetTransactionResponse {
    pub amount: f64,
    pub fee: Option<f64>,
    pub confirmations: u64,
    pub generated: Option<bool>,
    pub trusted: Option<bool>,
    pub blockhash: Option<String>,
    pub blockheight: Option<u32>,
    pub blockindex: Option<u32>,
    pub blocktime: Option<u64>,
    pub txid: String,
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
    pub parent_descs: Option<Vec<String>>,
    pub details: Vec<TransactionDetail>,
    pub hex: String,
    pub decoded: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct TransactionDetail {
    #[serde(rename = "involvesWatchonly")]
    involves_watchonly: Option<bool>,
    address: Option<String>,
    category: String,
    amount: f64,
    label: Option<String>,
    vout: u32,
    fee: Option<f64>,
    abandoned: Option<bool>,
    parent_descs: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct RawUTXO {
    pub txid: String,
    pub vout: u32,
    pub address: String,
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: String,
    pub amount: f64,
    pub confirmations: u64,
    pub spendable: bool,
    pub solvable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RpcBlockchainInfo {
    pub blocks: u64,
    pub headers: u64,
    bestblockhash: String,
    pub initialblockdownload: bool,
    pub warnings: String,
}

impl RpcBlockchainInfo {
    pub fn bestblockhash(&self) -> BlockHash {
        self.bestblockhash
            .parse::<BlockHash>()
            .expect("rpc: bad blockhash")
    }
}
