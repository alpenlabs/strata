use bitcoin::BlockHash;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};

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
    pub hex: String,
    pub decoded: Option<serde_json::Value>,
    // NOTE: "details" field omitted as not used, add it when used
}

#[derive(Deserialize)]
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
            write!(
                formatter,
                "a string representation of btc values with 8 decimal places"
            )
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let parts: Vec<&str> = v.split('.').collect();
            let combined = if parts.len() == 2 {
                let padded = format!("{:0<8}", parts[1]);
                if padded.len() != 8 {
                    return Err(E::custom(
                        "Invalid btc amount precision(more than 8 decimal places)",
                    ));
                }
                format!("{}{}", parts[0], padded)
            } else if parts.len() == 1 {
                format!("{}{}", parts[0], "0".repeat(8 as usize))
            } else {
                return Err(E::custom("Invalid amount representation"));
            };

            combined.parse().map_err(E::custom)
        }
    }
    deserializer.deserialize_any(SatVisitor)
}

#[cfg(test)]
mod test {

    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct TestStruct {
        #[serde(deserialize_with = "deserialize_satoshis")]
        value: u64,
    }

    #[test]
    fn test_deserialize_satoshis() {
        // Valid cases
        let json_data = r#"{"value": "0.000042"}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 4200);

        let json_data = r#"{"value": "1.23456789"}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 123456789);

        let json_data = r#"{"value": "123"}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 12300000000);

        let json_data = r#"{"value": "123.45"}"#;
        let result: TestStruct = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.value, 12345000000);

        // Invalid cases
        let json_data = r#"{"value": "123.456789012"}"#;
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
