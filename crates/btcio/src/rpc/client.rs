use std::sync::atomic::AtomicU64;
use std::time::Duration;
use std::{fmt::Display, str::FromStr};

#[cfg(test)]
use std::env;

use anyhow::anyhow;
use async_trait::async_trait;
use bitcoin::hashes::sha256d::Hash;
use bitcoin::hashes::Hash as _;
// use async_recursion::async_recursion;
use bitcoin::{
    block::{Header, Version},
    consensus::deserialize,
    hash_types::TxMerkleNode,
    hex::FromHex,
    Address, Block, BlockHash, CompactTarget, Network, Transaction,
};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, value::RawValue};
use tracing::*;

use super::traits::L1Client;
use super::types::{RawUTXO, RpcBlockchainInfo};

const MAX_RETRIES: u32 = 3;

// Represents a JSON-RPC error.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "RPCError code {}: {}", self.code, self.message)
    }
}

// Response is a struct that represents a response returned by the Bitcoin RPC
// It is generic over the type of the result field, which is usually a String in Bitcoin Core
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct Response<R> {
    pub result: Option<R>,
    pub error: Option<RpcError>,
    pub id: u64,
}

// BitcoinClient is a struct that represents a connection to a Bitcoin RPC node
#[derive(Debug)]
pub struct BitcoinClient {
    url: String,
    client: reqwest::Client,
    network: Network,
    next_id: AtomicU64,
}

impl BitcoinClient {
    pub fn new(url: String, username: String, password: String, network: Network) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!(
                "Basic {}",
                base64::encode(format!("{}:{}", username, password))
            )
            .parse()
            .expect("Failed to parse auth header!"),
        );
        headers.insert(
            "Content-Type",
            "application/json"
                .parse()
                .expect("Failed to parse content type header!"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build client!");

        Self {
            url,
            client,
            network,
            next_id: AtomicU64::new(0),
        }
    }

    fn next_id(&self) -> u64 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> anyhow::Result<T> {
        let mut retries = 0;
        loop {
            let id = self.next_id();
            let response = self
                .client
                .post(&self.url)
                .json(&json!({
                    "jsonrpc": "1.0",
                    "id": id,
                    "method": method,
                    "params": params
                }))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let data = resp.json::<Response<T>>().await?;
                    if let Some(err) = data.error {
                        return Err(anyhow::anyhow!(err));
                    }
                    return Ok(data.result.ok_or(anyhow!("Empty data received"))?);
                }
                Err(err) => {
                    warn!(err = %err, "Error calling bitcoin client");
                    if err.is_body() {
                        // Body error, unlikely to be recoverable by retrying
                        return Err(anyhow::anyhow!(err));
                    } else if err.is_status() {
                        // HTTP status error, not retryable
                        return Err(anyhow::anyhow!(err));
                    } else if err.is_decode() {
                        // Error decoding the response, retry might not help
                        return Err(anyhow::anyhow!(err));
                    } else if err.is_connect() {
                        // Connection error, retry might help
                    } else if err.is_timeout() {
                        // Timeout error, retry might help
                    } else if err.is_request() {
                        // General request error, retry might help
                    } else if err.is_builder() {
                        // Error building the request, unlikely to be recoverable
                        return Err(anyhow::anyhow!(err));
                    } else if err.is_redirect() {
                        // Redirect error, not retryable
                        return Err(anyhow::anyhow!(err));
                    } else {
                        // Unknown error, unlikely to be recoverable
                        return Err(anyhow::anyhow!(err));
                    }

                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(anyhow::anyhow!("Max retries exceeded: {}", err));
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            };
        }
    }

    /// get_block_count returns the current block height
    pub async fn get_block_count(&self) -> anyhow::Result<u64> {
        self.call::<u64>("getblockcount", &[]).await
    }

    /// This returns [(txid, timestamp)]
    pub async fn list_transactions(
        &self,
        confirmations: u32,
    ) -> anyhow::Result<Vec<(String, u64)>> {
        let res = self
            .call::<serde_json::Value>("listtransactions", &[to_value(confirmations)?])
            .await?;
        if let serde_json::Value::Array(array) = res {
            Ok(array
                .iter()
                .map(|el| {
                    (
                        serde_json::from_value::<String>(el.get("txid").unwrap().clone())
                            .unwrap()
                            .clone(),
                        serde_json::from_value::<u64>(el.get("time").unwrap().clone()).unwrap(),
                    )
                })
                .collect())
        } else {
            Err(anyhow!("Could not parse listransactions result"))
        }
    }

    /// get_mempool_txids returns a list of txids in the current mempool
    pub async fn get_mempool_txids(&self) -> anyhow::Result<Vec<String>> {
        let result = self
            .call::<Box<RawValue>>("getrawmempool", &[])
            .await?
            .to_string();

        serde_json::from_str::<Vec<String>>(&result).map_err(anyhow::Error::from)
    }

    /// get_block returns the block at the given hash
    pub async fn get_block(&self, hash: &BlockHash) -> anyhow::Result<Block> {
        let result = self
            .call::<Box<RawValue>>("getblock", &[to_value(hash.to_string())?, to_value(3)?])
            .await?
            .to_string();

        let full_block: serde_json::Value = serde_json::from_str(&result)?;

        let header: Header = Header {
            bits: CompactTarget::from_consensus(u32::from_str_radix(
                full_block["bits"].as_str().unwrap(),
                16,
            )?),
            merkle_root: TxMerkleNode::from_str(full_block["merkleroot"].as_str().unwrap())?,
            nonce: full_block["nonce"].as_u64().unwrap() as u32,
            prev_blockhash: full_block["previousblockhash"]
                .as_str()
                .and_then(|s| BlockHash::from_str(s).ok())
                .unwrap_or_else(|| BlockHash::from_raw_hash(Hash::all_zeros())),
            time: full_block["time"].as_u64().unwrap() as u32,
            version: Version::from_consensus(full_block["version"].as_u64().unwrap() as i32),
        };

        let txdata = full_block["tx"].as_array().unwrap();

        let txs: Vec<Transaction> = txdata
            .iter()
            .map(|tx| {
                let tx_hex = tx["hex"].as_str().unwrap();
                deserialize(&hex::decode(tx_hex).unwrap()).unwrap()
                // parse_hex_transaction(tx_hex).unwrap() // hex from rpc cannot be invalid
            })
            .collect();

        Ok(Block {
            header,
            txdata: txs,
        })
    }

    // get_utxos returns all unspent transaction outputs for the wallets of bitcoind
    pub async fn get_utxos(&self) -> anyhow::Result<Vec<RawUTXO>> {
        let utxos = self
            .call::<Vec<RawUTXO>>("listunspent", &[to_value(0)?, to_value(9999999)?])
            .await?;

        if utxos.is_empty() {
            return Err(anyhow!("No UTXOs found"));
        }

        Ok(utxos)
    }

    /// get number of confirmations for txid
    /// 0 confirmations means tx is still in mempool
    pub async fn get_transaction_confirmations(&self, txid: String) -> anyhow::Result<u64> {
        let result = self
            .call::<Box<RawValue>>("gettransaction", &[to_value(txid)?])
            .await?
            .to_string();
        let result: serde_json::Value = serde_json::from_str(&result)?;

        let confirmations = result.get("confirmations").unwrap().as_u64().unwrap();

        Ok(confirmations)
    }

    pub async fn list_since_block(&self, blockhash: String) -> anyhow::Result<Vec<String>> {
        let result = self
            .call::<Box<RawValue>>("listsinceblock", &[to_value(blockhash)?])
            .await?
            .to_string();

        let rawdata: serde_json::Value = serde_json::from_str(&result)?;
        let rawdata = rawdata.get("transactions").unwrap();
        let rawtxns = rawdata.as_array().unwrap();
        let txids = rawtxns
            .iter()
            .map(|x| x.get("txid").unwrap().as_str().unwrap().to_string())
            .collect();
        Ok(txids)
    }

    /// get_change_address returns a change address for the wallet of bitcoind
    async fn get_change_address(&self) -> anyhow::Result<Address> {
        let address_string = self.call::<String>("getrawchangeaddress", &[]).await?;
        Ok(Address::from_str(&address_string)?.require_network(self.network)?)
    }

    pub async fn get_change_addresses(&self) -> anyhow::Result<[Address; 2]> {
        let change_address = self.get_change_address().await?;
        let change_address_2 = self.get_change_address().await?;

        Ok([change_address, change_address_2])
    }

    // estimate_smart_fee estimates the fee to confirm a transaction in the next block
    pub async fn estimate_smart_fee(&self) -> anyhow::Result<f64> {
        let result = self
            .call::<Box<RawValue>>("estimatesmartfee", &[to_value(1)?])
            .await?
            .to_string();

        let result_map: serde_json::Value = serde_json::from_str(&result)?;

        // Issue: https://github.com/chainwayxyz/bitcoin-da/issues/3
        let btc_vkb = result_map
            .get("feerate")
            .unwrap_or(&serde_json::Value::from_str("0.00001").unwrap())
            .as_f64()
            .unwrap();

        // convert to sat/vB and round up
        Ok((btc_vkb * 100_000_000.0 / 1000.0).ceil())
    }

    // sign_raw_transaction_with_wallet signs a raw transaction with the wallet of bitcoind
    pub async fn sign_raw_transaction_with_wallet(&self, tx: String) -> anyhow::Result<String> {
        #[derive(Serialize, Deserialize, Debug)]
        struct SignError {
            txid: String,
            vout: u32,
            witness: Vec<String>,
            #[serde(rename = "scriptSig")]
            script_sig: String,
            sequence: u32,
            error: String,
        }

        #[derive(Serialize, Deserialize, Debug)]
        struct SignRPCResponse {
            hex: String,
            complete: bool,
            errors: Option<Vec<SignError>>,
        }

        let res = self
            .call::<SignRPCResponse>("signrawtransactionwithwallet", &[to_value(tx)?])
            .await?;

        match res.errors {
            None => Ok(res.hex),
            Some(ref errors) => {
                warn!("Error while signing with wallet: {:?}", res.errors);
                // concat all errors
                let err = errors
                    .iter()
                    .map(|x| x.error.clone())
                    .collect::<Vec<String>>()
                    .join(",");

                // TODO: This throws error even when a transaction is partially signed. There does
                // not seem to be other way to distinguish partially signed error from other
                // errors. So in future, we might need to handle that particular case where error
                // message is "CHECK(MULTI)SIG failing with non-zero signature (possibly need more
                // signatures)"
                Err(anyhow!(err))
            }
        }
    }

    // send_raw_transaction sends a raw transaction to the network
    pub async fn send_raw_transaction(&self, tx: String) -> anyhow::Result<Vec<u8>> {
        let resp = self
            .call::<String>("sendrawtransaction", &[to_value(tx)?])
            .await?;
        let hex = hex::decode(resp);
        match hex {
            Ok(hx) => Ok(hx),
            Err(e) => Err(anyhow!(e)),
        }
    }

    pub async fn list_wallets(&self) -> anyhow::Result<Vec<String>> {
        self.call::<Vec<String>>("listwallets", &[]).await
    }

    #[cfg(test)]
    pub async fn send_to_address(&self, address: String, amt: u32) -> anyhow::Result<String> {
        if self.network == Network::Regtest {
            let result = self
                .call::<Box<RawValue>>(
                    "sendtoaddress",
                    &[
                        to_value(address)?,
                        to_value(amt)?,
                        // All the following items are needed to pass the fee-rate and fee-rate
                        // needs to be passed just in case the regtest
                        // chain cannot estimate fee rate due to
                        // insufficient blocks
                        to_value("")?,
                        to_value("")?,
                        to_value(true)?,
                        to_value(true)?,
                        to_value(<Option<String>>::None)?,
                        to_value("unset")?,
                        to_value(<Option<String>>::None)?,
                        to_value(1.1)?, // fee rate
                    ],
                )
                .await;
            Ok(result.unwrap().to_string())
        } else {
            Err(anyhow!("Cannot send_to_address on non-regtest network"))
        }
    }
}

#[async_trait]
impl L1Client for BitcoinClient {
    async fn get_blockchain_info(&self) -> anyhow::Result<RpcBlockchainInfo> {
        let res = self
            .call::<RpcBlockchainInfo>("getblockchaininfo", &[])
            .await?;
        Ok(res)
    }

    // get_block_hash returns the block hash of the block at the given height
    async fn get_block_hash(&self, height: u64) -> anyhow::Result<BlockHash> {
        let hash = self
            .call::<String>("getblockhash", &[to_value(height)?])
            .await?;
        Ok(BlockHash::from_str(&hash)?)
    }

    async fn get_block_at(&self, height: u64) -> anyhow::Result<Block> {
        let hash = self.get_block_hash(height).await?;
        let block = self.get_block(&hash).await?;
        Ok(block)
    }
}

// TODO: Add functional tests
