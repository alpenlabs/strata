use serde::Deserialize;

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
