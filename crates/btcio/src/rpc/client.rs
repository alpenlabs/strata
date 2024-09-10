use std::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use bitcoin::{
    consensus::encode::serialize_hex, Address, Block, BlockHash, Network, Transaction, Txid,
};
use bitcoind_json_rpc_types::v26::{GetBlockVerbosityZero, GetBlockchainInfo, GetNewAddress};
use reqwest::{
    header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE},
    Client,
};
use serde::{de, Deserialize, Serialize};
use serde_json::{
    json,
    value::{RawValue, Value},
};
use tokio::time::sleep;
use tracing::*;

use crate::rpc::{
    error::{BitcoinRpcError, ClientError},
    traits::{Broadcaster, Reader, Signer, Wallet},
    types::{GetTransaction, ListTransactions, ListUnspent, SignRawTransactionWithWallet},
};

/// This is an alias for the result type returned by the [`BitcoinClient`].
pub type ClientResult<T> = Result<T, ClientError>;

/// The maximum number of retries for a request.
const MAX_RETRIES: u8 = 3;

/// Custom implementation to convert a value to a `Value` type.
pub fn to_value<T>(value: T) -> ClientResult<Value>
where
    T: Serialize,
{
    serde_json::to_value(value)
        .map_err(|e| ClientError::Param(format!("Error creating value: {}", e)))
}

/// An `async` client for interacting with a `bitcoind` instance.
#[derive(Debug)]
pub struct BitcoinClient {
    /// The URL of the `bitcoind` instance.
    url: String,
    /// The underlying `async` HTTP client.
    client: Client,
    /// The ID of the current request.
    id: AtomicUsize,
}

/// Response returned by the `bitcoind` RPC server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Response<R> {
    pub result: Option<R>,
    pub error: Option<BitcoinRpcError>,
    pub id: u64,
}

impl BitcoinClient {
    /// Creates a new [`BitcoinClient`] with the given URL, username, and password.
    pub fn new(url: String, username: String, password: String) -> ClientResult<Self> {
        if username.is_empty() || password.is_empty() {
            return Err(ClientError::MissingUserPassword);
        }

        let user_pw = general_purpose::STANDARD.encode(format!("{username}:{password}"));
        let authorization = format!("Basic {user_pw}")
            .parse()
            .map_err(|_| ClientError::Other("Error parsing header".to_string()))?;

        let content_type = "application/json"
            .parse()
            .map_err(|_| ClientError::Other("Error parsing header".to_string()))?;
        let headers =
            HeaderMap::from_iter([(AUTHORIZATION, authorization), (CONTENT_TYPE, content_type)]);

        trace!(headers = ?headers);

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| ClientError::Other(format!("Could not create client: {e}")))?;

        let id = AtomicUsize::new(0);

        trace!(url = %url, "Created bitcoin client");

        Ok(Self { url, client, id })
    }

    fn next_id(&self) -> usize {
        self.id.fetch_add(1, Ordering::AcqRel)
    }

    async fn call<T: de::DeserializeOwned + fmt::Debug>(
        &self,
        method: &str,
        params: &[Value],
    ) -> ClientResult<T> {
        let mut retries = 0;
        loop {
            trace!(%method, ?params, %retries, "Calling bitcoin client");

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
            trace!(?response, "Response received");
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
                        // Body error is unrecoverable
                        return Err(ClientError::Body(err.to_string()));
                    } else if err.is_status() {
                        // Status error is unrecoverable
                        let e = match err.status() {
                            Some(code) => ClientError::Status(code.to_string(), err.to_string()),
                            _ => ClientError::Other(err.to_string()),
                        };
                        return Err(e);
                    } else if err.is_decode() {
                        // Error decoding response, might be recoverable
                        let e = ClientError::MalformedResponse(err.to_string());
                        warn!(%e, "decoding error, retrying...");
                    } else if err.is_connect() {
                        // Connection error, might be recoverable
                        let e = ClientError::Connection(err.to_string());
                        warn!(%e, "connection error, retrying...");
                    } else if err.is_timeout() {
                        // Timeout error, might be recoverable
                        let e = ClientError::Timeout;
                        warn!(%e, "timeout error, retrying...");
                    } else if err.is_request() {
                        // General request error, might be recoverable
                        let e = ClientError::Request(err.to_string());
                        warn!(%e, "request error, retrying...");
                    } else if err.is_builder() {
                        // Request builder error is unrecoverable
                        return Err(ClientError::ReqBuilder(err.to_string()));
                    } else if err.is_redirect() {
                        // Redirect error is unrecoverable
                        return Err(ClientError::HttpRedirect(err.to_string()));
                    } else {
                        // Unknown error is unrecoverable
                        return Err(ClientError::Other("Unknown error".to_string()));
                    }
                }
            }
            retries += 1;
            if retries >= MAX_RETRIES {
                return Err(ClientError::MaxRetriesExceeded(MAX_RETRIES));
            }
            sleep(Duration::from_millis(1_000)).await;
        }
    }
}

#[async_trait]
impl Reader for BitcoinClient {
    async fn estimate_smart_fee(&self, conf_target: u16) -> ClientResult<u64> {
        let result = self
            .call::<Box<RawValue>>("estimatesmartfee", &[to_value(conf_target)?])
            .await?
            .to_string();

        let result_map: Value = result.parse::<Value>()?;

        let btc_vkb = result_map
            .get("feerate")
            .unwrap_or(&"0.00001".parse::<Value>().unwrap())
            .as_f64()
            .unwrap();

        // convert to sat/vB and round up
        Ok((btc_vkb * 100_000_000.0 / 1000.0) as u64)
    }

    async fn get_block(&self, hash: &BlockHash) -> ClientResult<Block> {
        let get_block = self
            .call::<GetBlockVerbosityZero>("getblock", &[to_value(hash.to_string())?, to_value(0)?])
            .await
            .expect("get_block failed");
        let block = get_block.block().expect("block failed");
        Ok(block)
    }

    async fn get_block_at(&self, height: u64) -> ClientResult<Block> {
        let hash = self.get_block_hash(height).await?;
        self.get_block(&hash).await
    }

    async fn get_block_count(&self) -> ClientResult<u64> {
        self.call::<u64>("getblockcount", &[]).await
    }

    async fn get_block_hash(&self, height: u64) -> ClientResult<BlockHash> {
        self.call::<BlockHash>("getblockhash", &[to_value(height)?])
            .await
    }

    async fn get_blockchain_info(&self) -> ClientResult<GetBlockchainInfo> {
        self.call::<GetBlockchainInfo>("getblockchaininfo", &[])
            .await
    }

    async fn get_raw_mempool(&self) -> ClientResult<Vec<Txid>> {
        self.call::<Vec<Txid>>("getrawmempool", &[]).await
    }

    async fn network(&self) -> ClientResult<Network> {
        Ok(self
            .call::<GetBlockchainInfo>("getblockchaininfo", &[])
            .await?
            .chain
            .parse::<Network>()
            .map_err(|e| ClientError::Parse(e.to_string()))?)
    }
}

#[async_trait]
impl Broadcaster for BitcoinClient {
    async fn send_raw_transaction(&self, tx: &Transaction) -> ClientResult<Txid> {
        let txstr = serialize_hex(tx);
        trace!(txstr = %txstr, "Sending raw transaction");
        match self
            .call::<Txid>("sendrawtransaction", &[to_value(txstr)?])
            .await
        {
            Ok(txid) => {
                trace!(?txid, "Transaction sent");
                Ok(txid)
            }
            Err(ClientError::Server(i, s)) => match i {
                // Dealing with known and common errors
                -27 => Ok(tx.compute_txid()), // Tx already in chain
                -25 => Err(ClientError::MalformedResponse(s)), // unrecoverable
                _ => Err(ClientError::Server(i, s)),
            },
            Err(e) => Err(ClientError::Other(e.to_string())),
        }
    }
}

#[async_trait]
impl Wallet for BitcoinClient {
    async fn get_new_address(&self) -> ClientResult<Address> {
        let address_unchecked = self
            .call::<GetNewAddress>("getnewaddress", &[])
            .await?
            .0
            .parse::<Address<_>>()
            .map_err(|e| ClientError::Parse(e.to_string()))?
            .assume_checked();
        Ok(address_unchecked)
    }
    async fn get_transaction(&self, txid: &Txid) -> ClientResult<GetTransaction> {
        Ok(self
            .call::<GetTransaction>("gettransaction", &[to_value(txid.to_string())?])
            .await?)
    }

    async fn get_utxos(&self) -> ClientResult<Vec<ListUnspent>> {
        let resp = self.call::<Vec<ListUnspent>>("listunspent", &[]).await?;
        trace!(?resp, "Got UTXOs");
        Ok(resp)
    }

    async fn list_transactions(&self, count: Option<usize>) -> ClientResult<Vec<ListTransactions>> {
        self.call::<Vec<ListTransactions>>("listtransactions", &[to_value(count)?])
            .await
    }

    async fn list_wallets(&self) -> ClientResult<Vec<String>> {
        self.call::<Vec<String>>("listwallets", &[]).await
    }
}

#[async_trait]
impl Signer for BitcoinClient {
    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: &Transaction,
    ) -> ClientResult<SignRawTransactionWithWallet> {
        let tx_hex = serialize_hex(tx);
        trace!(tx_hex = %tx_hex, "Signing transaction");
        self.call::<SignRawTransactionWithWallet>(
            "signrawtransactionwithwallet",
            &[to_value(tx_hex)?],
        )
        .await
    }
}

#[cfg(test)]
mod test {
    use std::env::set_var;

    use alpen_express_common::logging;
    use bitcoin::{consensus, hashes::Hash};
    use bitcoind::{bitcoincore_rpc::RpcApi, BitcoinD};

    use super::*;

    /// Get the authentication credentials for a given `bitcoind` instance.
    fn get_auth(bitcoind: &BitcoinD) -> (String, String) {
        let params = &bitcoind.params;
        let cookie_values = params.get_cookie_values().unwrap().unwrap();
        (cookie_values.user, cookie_values.password)
    }

    /// Mine a number of blocks of a given size `count`, which may be specified to a given coinbase
    /// `address`.
    pub fn mine_blocks(
        bitcoind: &BitcoinD,
        count: usize,
        address: Option<Address>,
    ) -> anyhow::Result<Vec<BlockHash>> {
        let coinbase_address = match address {
            Some(address) => address,
            None => bitcoind
                .client
                .get_new_address(None, None)?
                .assume_checked(),
        };
        let block_hashes = bitcoind
            .client
            .generate_to_address(count as _, &coinbase_address)?;
        Ok(block_hashes)
    }

    #[tokio::test()]
    async fn client_works() {
        logging::init();
        // setting the ENV variable `BITCOIN_XPRIV_RETRIEVABLE` to retrieve the xpriv
        set_var("BITCOIN_XPRIV_RETRIEVABLE", "true");
        let bitcoind = BitcoinD::from_downloaded().unwrap();
        let url = bitcoind.rpc_url();
        let (user, password) = get_auth(&bitcoind);
        let client = BitcoinClient::new(url, user, password).unwrap();

        // network
        let got = client.network().await.unwrap();
        let expected = Network::Regtest;

        assert_eq!(expected, got);
        // get_blockchain_info
        let get_blockchain_info = client.get_blockchain_info().await.unwrap();
        assert_eq!(get_blockchain_info.blocks, 0);

        let blocks = mine_blocks(&bitcoind, 101, None).unwrap();

        // get_block
        let expected = blocks.last().unwrap();
        let got = client.get_block(expected).await.unwrap().block_hash();
        assert_eq!(*expected, got);

        // get_block_at
        let target_height = blocks.len() as u64;
        let expected = blocks.last().unwrap();
        let got = client
            .get_block_at(target_height)
            .await
            .unwrap()
            .block_hash();
        assert_eq!(*expected, got);

        // get_block_count
        let expected = blocks.len() as u64;
        let got = client.get_block_count().await.unwrap();
        assert_eq!(expected, got);

        // get_block_hash
        let target_height = blocks.len() as u64;
        let expected = blocks.last().unwrap();
        let got = client.get_block_hash(target_height).await.unwrap();
        assert_eq!(*expected, got);

        // get_new_address
        let address = client.get_new_address().await.unwrap();
        let txid = client
            .call::<String>(
                "sendtoaddress",
                &[to_value(address.to_string()).unwrap(), to_value(1).unwrap()],
            )
            .await
            .unwrap()
            .parse::<Txid>()
            .unwrap();

        // get_transaction
        let tx = client.get_transaction(&txid).await.unwrap().hex;
        let got = client.send_raw_transaction(&tx).await.unwrap();
        let expected = txid;
        assert_eq!(expected, got);

        // get_raw_mempool
        let got = client.get_raw_mempool().await.unwrap();
        let expected = vec![txid];
        assert_eq!(expected, got);

        // estimate_smart_fee
        let got = client.estimate_smart_fee(1).await.unwrap();
        let expected = 1; // 1 sat/vB
        assert_eq!(expected, got);

        // sign_raw_transaction_with_wallet
        let got = client.sign_raw_transaction_with_wallet(&tx).await.unwrap();
        assert!(got.complete);
        assert!(consensus::encode::deserialize_hex::<Transaction>(&got.hex).is_ok());

        // send_raw_transaction
        let got = client.send_raw_transaction(&tx).await.unwrap();
        assert!(got.as_byte_array().len() == 32);

        // list_transactions
        let got = client.list_transactions(None).await.unwrap();
        assert_eq!(got.len(), 10);

        // get_utxos
        // let's mine one more block
        mine_blocks(&bitcoind, 1, None).unwrap();
        let got = client.get_utxos().await.unwrap();
        assert_eq!(got.len(), 3);
    }
}
