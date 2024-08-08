use alpen_test_utils::ArbitraryGenerator;
use async_trait::async_trait;
use bitcoin::{consensus::deserialize, hashes::Hash, Block, BlockHash, Network, Transaction, Txid};

use crate::rpc::{
    traits::{L1Client, SeqL1Client},
    types::{RawUTXO, RpcBlockchainInfo},
    ClientError,
};

pub struct TestBitcoinClient {
    pub confs: u64,
}

impl TestBitcoinClient {
    pub fn new(confs: u64) -> Self {
        Self { confs }
    }
}

const TEST_BLOCKSTR: &str = "000000207d862a78fcb02ab24ebd154a20b9992af6d2f0c94d3a67b94ad5a0009d577e70769f3ff7452ea5dd469d7d99f200d083d020f1585e4bd9f52e9d66b23891a9c6c4ea5e66ffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff04025f0200ffffffff02205fa01200000000160014d7340213b180c97bd55fedd7312b7e17389cf9bf0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";

#[async_trait]
impl L1Client for TestBitcoinClient {
    async fn get_blockchain_info(&self) -> Result<RpcBlockchainInfo, ClientError> {
        Ok(ArbitraryGenerator::new().generate())
    }

    async fn get_block_at(&self, _height: u64) -> Result<Block, ClientError> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block)
    }

    // get_block_hash returns the block hash of the block at the given height
    async fn get_block_hash(&self, _h: u64) -> Result<BlockHash, ClientError> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block.block_hash())
    }

    // send_raw_transaction sends a raw transaction to the network
    async fn send_raw_transaction<T: AsRef<[u8]> + Send>(
        &self,
        _tx: T,
    ) -> Result<Txid, ClientError> {
        Ok(Txid::from_slice(&[1u8; 32]).unwrap())
    }

    async fn get_transaction_confirmations<T: AsRef<[u8]> + Send>(
        &self,
        _txid: T,
    ) -> Result<u64, ClientError> {
        Ok(self.confs)
    }
}

#[async_trait]
impl SeqL1Client for TestBitcoinClient {
    // get_utxos returns all unspent transaction outputs for the wallets of bitcoind
    async fn get_utxos(&self) -> Result<Vec<RawUTXO>, ClientError> {
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

    async fn estimate_smart_fee(&self) -> Result<u64, ClientError> {
        Ok(3)
    }

    /// sign transaction with bitcoind wallet
    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: Transaction,
    ) -> Result<Transaction, ClientError> {
        Ok(tx)
    }

    fn network(&self) -> Network {
        Network::Regtest
    }
}
