#[cfg(test)]
use arbitrary::Arbitrary;
use bitcoin::BlockHash;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use tracing::*;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct RPCTransactionInfo {
    pub amount: f64,
    pub fee: Option<f64>,
    pub confirmations: u64,
    pub generated: Option<bool>,
    pub trusted: Option<bool>,
    pub blockhash: Option<String>,
    pub blockheight: Option<u64>,
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
    pub hex: String,
    // NOTE: "details", and "decoded" fields omitted as not used, add them when used
}

impl RPCTransactionInfo {
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

#[derive(Clone, Deserialize)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct RawUTXO {
    pub txid: String,
    pub vout: u32,
    pub address: String,
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: String,
    #[serde(deserialize_with = "deserialize_satoshis")]
    pub amount: u64, // satoshis
    pub confirmations: u64,
    pub spendable: bool,
    pub solvable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(Arbitrary))]
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

fn deserialize_satoshis<'d, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'d>,
{
    struct SatVisitor;

    impl<'d> Visitor<'d> for SatVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "a float representation of btc values expected")
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let sats = (v * 100_000_000.0).round() as u64;
            Ok(sats)
        }
    }
    deserializer.deserialize_any(SatVisitor)
}

#[cfg(test)]
mod test {

    use serde::Deserialize;

    use super::*;

    #[derive(Deserialize)]
    struct TestStruct {
        #[serde(deserialize_with = "deserialize_satoshis")]
        value: u64,
    }

    #[test]
    fn test_deserialize_satoshis() {
        // Valid cases
        let json_data = r#"{"value": 0.000042}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 4200);

        let json_data = r#"{"value": 1.23456789}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 123456789);

        let json_data = r#"{"value": 123.0}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 12300000000);

        let json_data = r#"{"value": 123.45}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 12345000000);

        // Invalid cases
        let json_data = r#"{"value": 123}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json_data);
        assert!(result.is_err());

        let json_data = r#"{"value": "abc"}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json_data);
        assert!(result.is_err());

        let json_data = r#"{"value": "123.456.78"}"#;
        let result: Result<TestStruct, _> = serde_json::from_str(json_data);
        assert!(result.is_err());
    }
}
