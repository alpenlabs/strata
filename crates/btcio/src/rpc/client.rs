use std::sync::atomic::AtomicU64;
use std::time::Duration;
use std::{fmt::Display, str::FromStr};

use async_trait::async_trait;
use bitcoin::consensus::encode::{deserialize_hex, serialize_hex};
use bitcoin::Txid;

use base64::engine::general_purpose;
use base64::Engine;
use bitcoin::{
    block::{Header, Version},
    consensus::deserialize,
    hash_types::TxMerkleNode,
    hashes::Hash as _,
    Address, Block, BlockHash, CompactTarget, Network, Transaction,
};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, value::RawValue, value::Value};
use thiserror::Error;
use tracing::*;

use super::types::{RawUTXO, RpcBlockchainInfo};
use super::{traits::BitcoinClient, types::GetTransactionResponse};

const MAX_RETRIES: u32 = 3;

pub fn to_val<T>(value: T) -> ClientResult<Value>
where
    T: Serialize,
{
    to_value(value).map_err(|e| ClientError::Param(format!("Error creating value: {}", e)))
}

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
pub struct BitcoinDClient {
    url: String,
    client: reqwest::Client,
    network: Network,
    next_id: AtomicU64,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Network: {0}")]
    Network(String),

    #[error("RPC server returned error '{1}' (code {0})")]
    Server(i32, String),

    #[error("Error parsing rpc response: {0}")]
    Parse(String),

    #[error("Could not create RPC Param")]
    Param(String),

    #[error("{0}")]
    Body(String),

    #[error("Obtained failure status({0}): {1}")]
    Status(StatusCode, String),

    #[error("Malformed Response: {0}")]
    MalformedResponse(String),

    #[error("Could not connect: {0}")]
    Connection(String),

    #[error("Timeout")]
    Timeout,

    #[error("HttpRedirect: {0}")]
    HttpRedirect(String),

    #[error("Could not build request: {0}")]
    ReqBuilder(String),

    #[error("Max retries {0} exceeded")]
    MaxRetriesExceeded(u32),

    #[error("Could not create request: {0}")]
    Request(String),

    #[error("Network address: {0}")]
    WrongNetworkAddress(Network),

    #[error("Could not sign")]
    Signing(Vec<String>),

    #[error("{0}")]
    Other(String),
}

impl From<serde_json::error::Error> for ClientError {
    fn from(value: serde_json::error::Error) -> Self {
        Self::Parse(format!("Could not parse {}", value))
    }
}

type ClientResult<T> = Result<T, ClientError>;

impl BitcoinDClient {
    pub fn new(url: String, username: String, password: String, network: Network) -> Self {
        let mut headers = HeaderMap::new();
        let mut user_pw = String::new();
        general_purpose::STANDARD.encode_string(format!("{}:{}", username, password), &mut user_pw);

        headers.insert(
            "Authorization",
            format!("Basic {}", user_pw)
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

    pub fn network(&self) -> Network {
        self.network
    }

    fn next_id(&self) -> u64 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel)
    }

    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> ClientResult<T> {
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
                    let data = resp
                        .json::<Response<T>>()
                        .await
                        .map_err(|e| ClientError::Parse(e.to_string()))?;
                    if let Some(err) = data.error {
                        return Err(ClientError::Server(err.code, err.message));
                    }
                    return data
                        .result
                        .ok_or_else(|| ClientError::Other("Empty data received".to_string()));
                }
                Err(err) => {
                    warn!(err = %err, "Error calling bitcoin client");

                    if err.is_body() {
                        // Body error, unlikely to be recoverable by retrying
                        return Err(ClientError::Body(err.to_string()));
                    } else if err.is_status() {
                        // HTTP status error, not retryable
                        let e = match err.status() {
                            Some(code) => ClientError::Status(code, err.to_string()),
                            _ => ClientError::Other(err.to_string()),
                        };
                        return Err(e);
                    } else if err.is_decode() {
                        // Error decoding the response, retry might not help
                        return Err(ClientError::MalformedResponse(err.to_string()));
                    } else if err.is_connect() {
                        // Connection error, retry might help
                        let e = ClientError::Connection(err.to_string());
                        warn!(%e, "connection error, retrying...");
                    } else if err.is_timeout() {
                        let e = ClientError::Timeout;
                        // Timeout error, retry might help
                        warn!(%e, "timeout error, retrying...");
                    } else if err.is_request() {
                        // General request error, retry might help
                        let e = ClientError::Request(err.to_string());
                        warn!(%e, "request error, retrying...");
                    } else if err.is_builder() {
                        // Error building the request, unlikely to be recoverable
                        return Err(ClientError::ReqBuilder(err.to_string()));
                    } else if err.is_redirect() {
                        // Redirect error, not retryable
                        return Err(ClientError::HttpRedirect(err.to_string()));
                    } else {
                        // Unknown error, unlikely to be recoverable
                        return Err(ClientError::Other("Unknown error".to_string()));
                    }

                    retries += 1;
                    if retries >= MAX_RETRIES {
                        return Err(ClientError::MaxRetriesExceeded(MAX_RETRIES));
                    }
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            };
        }
    }

    // get_block_count returns the current block height
    pub async fn get_block_count(&self) -> ClientResult<u64> {
        self.call::<u64>("getblockcount", &[]).await
    }

    // This returns [(txid, timestamp)]
    pub async fn list_transactions(&self, confirmations: u32) -> ClientResult<Vec<(String, u64)>> {
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
            Err(ClientError::MalformedResponse(res.to_string()))
        }
    }

    // get_mempool_txids returns a list of txids in the current mempool
    pub async fn get_mempool_txids(&self) -> ClientResult<Vec<String>> {
        let result = self
            .call::<Box<RawValue>>("getrawmempool", &[])
            .await?
            .to_string();

        serde_json::from_str::<Vec<String>>(&result)
            .map_err(|e| ClientError::MalformedResponse(e.to_string()))
    }

    // get_block returns the block at the given hash
    pub async fn get_block(&self, hash: BlockHash) -> ClientResult<Block> {
        let result = self
            .call::<Box<RawValue>>("getblock", &[to_value(hash.to_string())?, to_value(3)?])
            .await?
            .to_string();

        let full_block: serde_json::Value = serde_json::from_str(&result)?;

        let header: anyhow::Result<Header> = (|| {
            Ok(Header {
                bits: CompactTarget::from_consensus(u32::from_str_radix(
                    full_block["bits"].as_str().unwrap(),
                    16,
                )?),
                merkle_root: TxMerkleNode::from_str(full_block["merkleroot"].as_str().unwrap())?,
                nonce: full_block["nonce"].as_u64().unwrap() as u32,
                prev_blockhash: BlockHash::from_str(
                    full_block["previousblockhash"].as_str().unwrap(),
                )?,
                time: full_block["time"].as_u64().unwrap() as u32,
                version: Version::from_consensus(full_block["version"].as_u64().unwrap() as i32),
            })
        })();
        let header = header.map_err(|e| ClientError::Other(e.to_string()))?;

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

    pub async fn list_since_block(&self, blockhash: String) -> ClientResult<Vec<String>> {
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

    // get_change_address returns a change address for the wallet of bitcoind
    async fn get_change_address(&self) -> ClientResult<Address> {
        let address_string = self.call::<String>("getrawchangeaddress", &[]).await?;
        let addr = Address::from_str(&address_string).and_then(|x| x.require_network(self.network));
        addr.map_err(|_| ClientError::WrongNetworkAddress(self.network))
    }

    pub async fn get_change_addresses(&self) -> ClientResult<[Address; 2]> {
        let change_address = self.get_change_address().await?;
        let change_address_2 = self.get_change_address().await?;

        Ok([change_address, change_address_2])
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
            Err(anyhow::anyhow!(
                "Cannot send_to_address on non-regtest network"
            ))
        }
    }
    pub async fn list_wallets(&self) -> ClientResult<Vec<String>> {
        self.call::<Vec<String>>("listwallets", &[]).await
    }
}

#[async_trait]
impl BitcoinClient for BitcoinDClient {
    async fn get_blockchain_info(&self) -> ClientResult<RpcBlockchainInfo> {
        let res = self
            .call::<RpcBlockchainInfo>("getblockchaininfo", &[])
            .await?;
        Ok(res)
    }

    // get_block_hash returns the block hash of the block at the given height
    async fn get_block_hash(&self, height: u64) -> ClientResult<BlockHash> {
        let hash = self
            .call::<String>("getblockhash", &[to_value(height)?])
            .await?;
        Ok(
            BlockHash::from_str(&hash)
                .map_err(|e| ClientError::MalformedResponse(e.to_string()))?,
        )
    }

    async fn get_block_at(&self, height: u64) -> ClientResult<Block> {
        let hash = self.get_block_hash(height).await?;
        let block = self.get_block(hash).await?;
        Ok(block)
    }

    // send_raw_transaction sends a raw transaction to the network
    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(&self, tx: T) -> ClientResult<Txid> {
        let txstr = hex::encode(tx);
        let resp = self
            .call::<String>("sendrawtransaction", &[to_value(txstr)?])
            .await?;

        let hex = hex::decode(resp.clone());
        match hex {
            Ok(hx) => {
                if hx.len() != 32 {
                    return Err(ClientError::MalformedResponse(resp));
                }
                let mut arr: [u8; 32] = [0; 32];
                arr.copy_from_slice(&hx);
                Ok(Txid::from_slice(&arr)
                    .map_err(|e| ClientError::MalformedResponse(e.to_string()))?)
            }
            Err(e) => Err(ClientError::MalformedResponse(e.to_string())),
        }
    }

    async fn get_transaction_confirmations<T: AsRef<[u8; 32]> + Send>(
        &self,
        txid: T,
    ) -> ClientResult<u64> {
        let mut txid = txid.as_ref().to_vec();
        txid.reverse();
        let txid = hex::encode(&txid);
        let result = self
            .call::<GetTransactionResponse>("gettransaction", &[to_val(txid)?])
            .await?;

        Ok(result.confirmations)
    }

    // get_utxos returns all unspent transaction outputs for the wallets of bitcoind
    async fn get_utxos(&self) -> ClientResult<Vec<RawUTXO>> {
        let utxos = self
            .call::<Vec<RawUTXO>>("listunspent", &[to_value(0)?, to_value(9999999)?])
            .await?;

        if utxos.is_empty() {
            return Err(ClientError::Other("No UTXOs found".to_string()));
        }

        Ok(utxos)
    }

    // estimate_smart_fee estimates the fee to confirm a transaction in the next block
    async fn estimate_smart_fee(&self) -> ClientResult<u64> {
        let result = self
            .call::<Box<RawValue>>("estimatesmartfee", &[to_value(1)?])
            .await?
            .to_string();

        let result_map: serde_json::Value = serde_json::from_str(&result)?;

        let btc_vkb = result_map
            .get("feerate")
            .unwrap_or(&serde_json::Value::from_str("0.00001").unwrap())
            .as_f64()
            .unwrap();

        // convert to sat/vB and round up
        Ok((btc_vkb * 100_000_000.0 / 1000.0).ceil() as u64)
    }

    // sign_raw_transaction_with_wallet signs a raw transaction with the wallet of bitcoind
    async fn sign_raw_transaction_with_wallet(&self, tx: Transaction) -> ClientResult<Transaction> {
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

        let txraw = serialize_hex(&tx);
        let res = self
            .call::<SignRPCResponse>("signrawtransactionwithwallet", &[to_value(txraw)?])
            .await?;

        match res.errors {
            None => {
                let hex = res.hex;
                let tx = deserialize_hex(&hex).map_err(|e| ClientError::Parse(e.to_string()))?;
                Ok(tx)
            }
            Some(ref errors) => {
                warn!("Error while signing with wallet: {:?}", res.errors);
                let errs = errors
                    .iter()
                    .map(|x| x.error.clone())
                    .collect::<Vec<String>>();

                // TODO: This throws error even when a transaction is partially signed. There does
                // not seem to be other way to distinguish partially signed error from other
                // errors. So in future, we might need to handle that particular case where error
                // message is "CHECK(MULTI)SIG failing with non-zero signature (possibly need more
                // signatures)"
                Err(ClientError::Signing(errs))
            }
        }
    }

    fn get_network(&self) -> Network {
        self.network
    }
}

// TODO: Add functional tests
