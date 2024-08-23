use alpen_test_utils::ArbitraryGenerator;
use async_trait::async_trait;
use bitcoin::{consensus::deserialize, hashes::Hash, Amount, Block, BlockHash, Transaction, Txid};
use bitcoincore_rpc_async::Error as RpcError;

use crate::rpc::{
    traits::BitcoinClient,
    types::{RawUTXO, RpcBlockchainInfo, RpcTransactionInfo},
};

pub struct BitcoinDTestClient {
    pub confs: u64,
    /// Parameter that indicates which height a transaction is included in
    pub included_height: u64,
}

impl BitcoinDTestClient {
    pub fn new(confs: u64) -> Self {
        Self {
            confs,
            included_height: 100, // Use arbitrary value, make configurable as necessary
        }
    }
}

const TEST_BLOCKSTR: &str = "000000207d862a78fcb02ab24ebd154a20b9992af6d2f0c94d3a67b94ad5a0009d577e70769f3ff7452ea5dd469d7d99f200d083d020f1585e4bd9f52e9d66b23891a9c6c4ea5e66ffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff04025f0200ffffffff02205fa01200000000160014d7340213b180c97bd55fedd7312b7e17389cf9bf0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";

#[async_trait]
impl BitcoinClient for BitcoinDTestClient {
    async fn estimate_smart_fee(&self, _conf_target: u16) -> Result<u64, RpcError> {
        Ok(3) // hardcoded to 3 sats/vByte
    }

    async fn get_block(&self, _hash: BlockHash) -> Result<Block, RpcError> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block)
    }

    async fn get_block_at(&self, _height: u64) -> Result<Block, RpcError> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block)
    }

    async fn get_block_count(&self) -> Result<u64, RpcError> {
        Ok(1)
    }

    async fn get_block_hash(&self, _height: u64) -> Result<BlockHash, RpcError> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block.block_hash())
    }

    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, RpcError> {
        Ok(ArbitraryGenerator::new().generate())
    }

    async fn get_new_address(&self) -> Result<String, RpcError> {
        // random regtest address from https://bitcoin.stackexchange.com/q/91222
        Ok("bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw".to_string())
    }

    async fn get_raw_mempool(&self) -> Result<Vec<Txid>, RpcError> {
        Ok(vec![])
    }

    async fn get_transaction(&self, _txid: Txid) -> Result<Transaction, RpcError> {
        let tx: Transaction = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(tx)
    }

    async fn get_transaction_confirmations<T: AsRef<[u8; 32]> + Send>(
        &self,
        _txid: T,
    ) -> Result<u64, RpcError> {
        Ok(self.confs)
    }

    async fn get_transaction_info(&self, _txid: Txid) -> Result<RpcTransactionInfo, RpcError> {
        let mut txinfo: RpcTransactionInfo = ArbitraryGenerator::new().generate();
        txinfo.confirmations = self.confs;
        txinfo.blockheight = Some(self.included_height);
        Ok(txinfo)
    }

    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, RpcError> {
        // Generate enough utxos to cover for the costs later
        let utxos: Vec<_> = (1..10)
            .map(|_| ArbitraryGenerator::new().generate())
            .enumerate()
            .map(|(i, x)| RawUTXO {
                txid: hex::encode([i as u8; 32]), // need to do this otherwise random str is
                // generated
                amount: 100 * 100_000_000,
                spendable: true,
                solvable: true,
                ..x
            })
            .collect();
        Ok(utxos)
    }

    async fn list_since_block(
        &self,
        _blockhash: BlockHash,
    ) -> Result<Vec<(String, u64)>, RpcError> {
        Ok(vec![])
    }

    async fn list_transactions(&self, _count: Option<u32>) -> Result<Vec<(String, u64)>, RpcError> {
        Ok(vec![])
    }

    async fn list_wallets(&self) -> Result<Vec<String>, RpcError> {
        Ok(vec![])
    }

    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(&self, _tx: T) -> Result<Txid, RpcError> {
        Ok(Txid::from_slice(&[1u8; 32]).unwrap())
    }

    async fn send_to_address(&self, _address: &str, _amount: Amount) -> Result<Txid, RpcError> {
        Ok(Txid::from_slice(&[0u8; 32]).unwrap())
    }

    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: Transaction,
    ) -> Result<Transaction, RpcError> {
        Ok(tx)
    }
}
