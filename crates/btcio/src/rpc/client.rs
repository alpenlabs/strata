use async_trait::async_trait;
use bitcoincore_rpc_async::bitcoin::{consensus, Address, Amount, Block, BlockHash, Transaction, Txid};
use bitcoincore_rpc_async::json::{
    AddressType, Bip125Replaceable, GetBlockchainInfoResult, GetTransactionResult,
    ListUnspentResultEntry,
};
use bitcoincore_rpc_async::{Auth, Client, RawTx};
use bitcoincore_rpc_async::{Error as RpcError, RpcApi};

use super::traits::BitcoinClient;
use super::types::{RawUTXO, RpcBlockchainInfo, RpcTransactionInfo};

/// Thin wrapper around the [`bitcoincore_rpc_async`]'s [`Client`].
///
/// Provides a simple interface to interact asynchronously with a
/// Bitcoin Core node via JSON-RPC.
#[derive(Debug)]
pub struct BitcoinDClient(Client);

impl BitcoinDClient {
    /// Creates a new [`BitcoinDClient`] instance.
    ///
    /// # Note
    ///
    /// The only supported [`Auth`] method is [`UserPass`](Auth::UserPass),
    /// by providing a `username` and `password`.
    pub async fn new(url: String, username: String, password: String) -> Result<Self, RpcError> {
        let auth = Auth::UserPass(username, password);
        Ok(BitcoinDClient(Client::new(url, auth).await?))
    }
}

#[async_trait]
impl BitcoinClient for BitcoinDClient {
    async fn estimate_smart_fee(&self, conf_target: u16) -> Result<Option<Amount>, RpcError> {
        let fee = self.0.estimate_smart_fee(conf_target, None).await?;
        Ok(fee.fee_rate)
    }

    async fn get_block(&self, hash: &BlockHash) -> Result<Block, RpcError> {
        self.0.get_block(hash).await
    }

    async fn get_block_at(&self, height: u64) -> Result<Block, RpcError> {
        let hash = self.0.get_block_hash(height).await?;
        let block = self.0.get_block(&hash).await?;
        Ok(block)
    }

    async fn get_block_count(&self) -> Result<u64, RpcError> {
        self.0.get_block_count().await
    }

    async fn get_block_hash(&self, height: u64) -> Result<BlockHash, RpcError> {
        self.0.get_block_hash(height).await
    }

    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, RpcError> {
        Ok(self.0.get_blockchain_info().await?.into())
    }

    async fn get_new_address(
        &self,
        address_type: Option<AddressType>,
    ) -> Result<Address, RpcError> {
        self.0.get_new_address(None, address_type).await
    }

    async fn get_raw_mempool(&self) -> Result<Vec<Txid>, RpcError> {
        self.0.get_raw_mempool().await
    }

    async fn get_transaction(&self, txid: &Txid) -> Result<Transaction, RpcError> {
        let rpc_transaction = self.0.get_transaction(txid, None).await?;
        Ok(rpc_transaction.transaction()?)
    }

    async fn get_transaction_info(&self, txid: &Txid) -> Result<RpcTransactionInfo, RpcError> {
        Ok(self.0.get_transaction(txid, None).await?.into())
    }

    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, RpcError> {
        Ok(self
            .0
            .list_unspent(Some(0), Some(9_999_999), None, None, None)
            .await?
            .into_iter()
            .map(|u| u.into())
            .collect())
    }

    async fn list_since_block(&self, block_hash: &BlockHash) -> Result<Vec<Txid>, RpcError> {
        Ok(self
            .0
            .list_since_block(Some(block_hash), None, None, None)
            .await?
            .transactions
            .into_iter()
            .map(|t| t.info.txid)
            .collect())
    }

    async fn list_transactions(&self, count: Option<usize>) -> Result<Vec<(Txid, u64)>, RpcError> {
        Ok(self
            .0
            .list_transactions(None, count, None, None)
            .await?
            .into_iter()
            .map(|t| (t.info.txid, t.info.time))
            .collect())
    }

    async fn list_wallets(&self) -> Result<Vec<String>, RpcError> {
        self.0.list_wallets().await
    }

    async fn send_raw_transaction<T: Sync + Send + RawTx>(&self, tx: T) -> Result<Txid, RpcError> {
        self.0.send_raw_transaction(tx).await
    }

    async fn send_to_address(&self, address: &Address, amount: Amount) -> Result<Txid, RpcError> {
        self.0
            .send_to_address(address, amount, None, None, None, None, None, None)
            .await
    }

    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: &Transaction,
    ) -> Result<Transaction, RpcError> {
        let bytes = self
            .0
            .sign_raw_transaction_with_wallet(tx, None, None)
            .await?
            .hex;
        let tx = consensus::deserialize(&bytes).expect("rpc: bad transaction");
        Ok(tx)
    }
}

impl From<GetBlockchainInfoResult> for RpcBlockchainInfo {
    fn from(info: GetBlockchainInfoResult) -> Self {
        Self {
            blocks: info.blocks,
            headers: info.headers,
            bestblockhash: info.best_block_hash.to_string(),
            initialblockdownload: info.initial_block_download,
            warnings: info.warnings,
        }
    }
}

impl From<GetTransactionResult> for RpcTransactionInfo {
    fn from(original: GetTransactionResult) -> Self {
        let tx = original.transaction().expect("rpc: bad transaction");
        Self {
            amount: original.amount.as_btc(),
            fee: Some(original.fee.expect("rpc: bad fee").as_btc()),
            confirmations: original.info.confirmations as u64,
            blockhash: original.info.blockhash.map(|bh| bh.to_string()),
            blockheight: original.info.blockheight.map(|bh| bh as u64),
            blockindex: original.info.blockindex.map(|bi| bi as u32),
            blocktime: original.info.blocktime.map(|bt| bt as u64),
            txid: original.info.txid.to_string(),
            from: Some(
                tx.input
                    .iter()
                    .map(|i| i.previous_output.to_string())
                    .collect(),
            ),
            time: original.info.time,
            timereceived: original.info.timereceived as u64,
            bip125_replaceable: match original.info.bip125_replaceable {
                Bip125Replaceable::Yes => "yes".to_owned(),
                _ => "no".to_owned(),
            },
            hex: consensus::encode::serialize_hex(&original.hex),
        }
    }
}

impl From<ListUnspentResultEntry> for RawUTXO {
    fn from(original: ListUnspentResultEntry) -> Self {
        Self {
            txid: original.txid.to_string(),
            vout: original.vout,
            address: original.address.unwrap().to_string(),
            script_pub_key: original.script_pub_key.to_string(),
            amount: original.amount.as_btc() as u64,
            confirmations: original.confirmations as u64,
            spendable: original.spendable,
            solvable: original.solvable,
        }
    }
}
