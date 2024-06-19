use bitcoin::BlockHash;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RawUTXO {
    pub txid: String,
    pub vout: u32,
    pub address: String,
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: String,
    pub amount: f64, // btcs not satoshis
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
