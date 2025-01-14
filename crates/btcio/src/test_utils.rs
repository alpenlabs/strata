use async_trait::async_trait;
use bitcoin::{
    bip32::Xpriv,
    consensus::{self, deserialize},
    hashes::Hash,
    taproot::ControlBlock,
    Address, Amount, Block, BlockHash, Network, ScriptBuf, SignedAmount, Transaction, Txid, Work,
};
use strata_l1tx::envelope::builder::build_envelope_script;
use strata_primitives::l1::payload::L1Payload;

use crate::{
    rpc::{
        traits::{BroadcasterRpc, ReaderRpc, SignerRpc, WalletRpc},
        types::{
            GetBlockchainInfo, GetTransaction, ImportDescriptor, ImportDescriptorResult,
            ListTransactions, ListUnspent, SignRawTransactionWithWallet,
        },
        ClientResult,
    },
    writer::builder::{build_reveal_transaction, EnvelopeError},
};

/// A test implementation of a Bitcoin client.
#[derive(Debug, Clone)]
pub struct TestBitcoinClient {
    /// Confirmations of a given transaction.
    pub confs: u64,
    /// Which height a transaction was included in.
    pub included_height: u64,
}

impl TestBitcoinClient {
    pub fn new(confs: u64) -> Self {
        Self {
            confs,
            // Use arbitrary value, make configurable as necessary
            included_height: 100,
        }
    }
}

const TEST_BLOCKSTR: &str = "000000207d862a78fcb02ab24ebd154a20b9992af6d2f0c94d3a67b94ad5a0009d577e70769f3ff7452ea5dd469d7d99f200d083d020f1585e4bd9f52e9d66b23891a9c6c4ea5e66ffff7f200000000001020000000001010000000000000000000000000000000000000000000000000000000000000000ffffffff04025f0200ffffffff02205fa01200000000160014d7340213b180c97bd55fedd7312b7e17389cf9bf0000000000000000266a24aa21a9ede2f61c3f71d1defd3fa999dfa36953755c690689799962b48bebd836974e8cf90120000000000000000000000000000000000000000000000000000000000000000000000000";

/// A test transaction.
///
/// # Note
///
/// Taken from
/// [`rust-bitcoin` test](https://docs.rs/bitcoin/0.32.1/src/bitcoin/blockdata/transaction.rs.html#1638).
pub const SOME_TX: &str = "0100000001a15d57094aa7a21a28cb20b59aab8fc7d1149a3bdbcddba9c622e4f5f6a99ece010000006c493046022100f93bb0e7d8db7bd46e40132d1f8242026e045f03a0efe71bbb8e3f475e970d790221009337cd7f1f929f00cc6ff01f03729b069a7c21b59b1736ddfee5db5946c5da8c0121033b9b137ee87d5a812d6f506efdd37f0affa7ffc310711c06c7f3e097c9447c52ffffffff0100e1f505000000001976a9140389035a9225b3839e2bbf32d826a1e222031fd888ac00000000";

#[async_trait]
impl ReaderRpc for TestBitcoinClient {
    async fn estimate_smart_fee(&self, _conf_target: u16) -> ClientResult<u64> {
        Ok(3)
    }

    async fn get_block(&self, _hash: &BlockHash) -> ClientResult<Block> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block)
    }

    async fn get_block_at(&self, _height: u64) -> ClientResult<Block> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block)
    }

    async fn get_block_count(&self) -> ClientResult<u64> {
        Ok(100)
    }

    // get_block_hash returns the block hash of the block at the given height
    async fn get_block_hash(&self, _h: u64) -> ClientResult<BlockHash> {
        let block: Block = deserialize(&hex::decode(TEST_BLOCKSTR).unwrap()).unwrap();
        Ok(block.block_hash())
    }

    async fn get_blockchain_info(&self) -> ClientResult<GetBlockchainInfo> {
        Ok(GetBlockchainInfo {
            chain: "regtest".to_string(),
            blocks: 100,
            headers: 100,
            best_block_hash: BlockHash::all_zeros().to_string(),
            difficulty: 1.0,
            median_time: 10 * 60,
            verification_progress: 1.0,
            initial_block_download: false,
            chain_work: Work::from_be_bytes([0; 32]).to_string(),
            size_on_disk: 1_000_000,
            pruned: false,
            prune_height: None,
            automatic_pruning: None,
            prune_target_size: None,
        })
    }

    async fn get_raw_mempool(&self) -> ClientResult<Vec<Txid>> {
        Ok(vec![])
    }

    async fn network(&self) -> ClientResult<Network> {
        Ok(Network::Regtest)
    }
}

#[async_trait]
impl BroadcasterRpc for TestBitcoinClient {
    // send_raw_transaction sends a raw transaction to the network
    async fn send_raw_transaction(&self, _tx: &Transaction) -> ClientResult<Txid> {
        Ok(Txid::from_slice(&[1u8; 32]).unwrap())
    }
}

#[async_trait]
impl WalletRpc for TestBitcoinClient {
    async fn get_new_address(&self) -> ClientResult<Address> {
        // taken from https://bitcoin.stackexchange.com/q/91222
        let addr = "bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw"
            .parse::<Address<_>>()
            .unwrap()
            .assume_checked();
        Ok(addr)
    }

    async fn get_transaction(&self, txid: &Txid) -> ClientResult<GetTransaction> {
        let some_tx = consensus::encode::deserialize_hex(SOME_TX).unwrap();
        Ok(GetTransaction {
            amount: SignedAmount::from_btc(100.0).unwrap(),
            confirmations: self.confs,
            generated: None,
            trusted: None,
            blockhash: None,
            blockheight: Some(self.included_height),
            blockindex: None,
            blocktime: None,
            txid: *txid,
            wtxid: txid.to_string(),
            walletconflicts: vec![],
            replaced_by_txid: None,
            replaces_txid: None,
            comment: None,
            to: None,
            time: 0,
            timereceived: 0,
            bip125_replaceable: "false".to_string(),
            details: vec![],
            hex: some_tx,
        })
    }

    async fn get_utxos(&self) -> ClientResult<Vec<ListUnspent>> {
        // plenty of sats
        (1..10)
            .map(|i| {
                Ok(ListUnspent {
                    txid: Txid::from_slice(&[i; 32]).unwrap(),
                    vout: 0,
                    address: "bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw"
                        .parse::<Address<_>>()
                        .unwrap(),
                    label: None,
                    script_pubkey: "foo".to_string(),
                    amount: Amount::from_btc(100.0).unwrap(),
                    confirmations: self.confs as u32,
                    spendable: true,
                    solvable: true,
                    safe: true,
                })
            })
            .collect()
    }

    async fn list_transactions(
        &self,
        _count: Option<usize>,
    ) -> ClientResult<Vec<ListTransactions>> {
        Ok(vec![])
    }

    async fn list_wallets(&self) -> ClientResult<Vec<String>> {
        Ok(vec![])
    }
}

#[async_trait]
impl SignerRpc for TestBitcoinClient {
    async fn sign_raw_transaction_with_wallet(
        &self,
        tx: &Transaction,
    ) -> ClientResult<SignRawTransactionWithWallet> {
        let tx_hex = consensus::encode::serialize_hex(tx);
        Ok(SignRawTransactionWithWallet {
            hex: tx_hex,
            complete: true,
            errors: None,
        })
    }
    async fn get_xpriv(&self) -> ClientResult<Option<Xpriv>> {
        // taken from https://docs.rs/bitcoin/0.32.2/src/bitcoin/bip32.rs.html#1090
        // DO NOT USE THIS BY ANY MEANS IN PRODUCTION WITH REAL FUNDS
        let xpriv = "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi".parse::<Xpriv>().unwrap();
        Ok(Some(xpriv))
    }

    async fn import_descriptors(
        &self,
        _descriptors: Vec<ImportDescriptor>,
        _wallet_name: String,
    ) -> ClientResult<Vec<ImportDescriptorResult>> {
        Ok(vec![ImportDescriptorResult { success: true }])
    }
}

pub fn generate_envelope_script_test(
    envelope_data: L1Payload,
    rollup_name: &str,
    version: u8,
) -> anyhow::Result<ScriptBuf> {
    build_envelope_script(&envelope_data, rollup_name, version)
}

pub fn build_reveal_transaction_test(
    input_transaction: Transaction,
    recipient: Address,
    output_value: u64,
    fee_rate: u64,
    reveal_script: &ScriptBuf,
    control_block: &ControlBlock,
) -> Result<Transaction, EnvelopeError> {
    build_reveal_transaction(
        input_transaction,
        recipient,
        output_value,
        fee_rate,
        reveal_script,
        control_block,
    )
}

#[cfg(test)]
pub mod corepc_node_helpers {
    use std::env;

    use bitcoin::{Address, BlockHash};
    use corepc_node::BitcoinD;

    use crate::rpc::BitcoinClient;

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
            None => bitcoind.client.new_address()?,
        };
        let block_hashes = bitcoind
            .client
            .generate_to_address(count as _, &coinbase_address)?
            .0
            .iter()
            .map(|hash| hash.parse::<BlockHash>())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(block_hashes)
    }

    pub fn get_bitcoind_and_client() -> (BitcoinD, BitcoinClient) {
        // setting the ENV variable `BITCOIN_XPRIV_RETRIEVABLE` to retrieve the xpriv
        env::set_var("BITCOIN_XPRIV_RETRIEVABLE", "true");
        let bitcoind = BitcoinD::from_downloaded().unwrap();
        let url = bitcoind.rpc_url();
        let (user, password) = get_auth(&bitcoind);
        let client = BitcoinClient::new(url, user, password).unwrap();
        (bitcoind, client)
    }
}

#[cfg(test)]
pub(crate) mod test_context {
    use std::sync::Arc;

    use bitcoin::{Address, Network};
    use strata_config::btcio::WriterConfig;
    use strata_status::StatusChannel;
    use strata_test_utils::{l2::gen_params, ArbitraryGenerator};

    use crate::{test_utils::TestBitcoinClient, writer::context::WriterContext};

    pub fn get_writer_context() -> Arc<WriterContext<TestBitcoinClient>> {
        let client = Arc::new(TestBitcoinClient::new(1));
        let addr = "bcrt1q6u6qyya3sryhh42lahtnz2m7zuufe7dlt8j0j5"
            .parse::<Address<_>>()
            .unwrap()
            .require_network(Network::Regtest)
            .unwrap();
        let cfg = Arc::new(WriterConfig::default());
        let status_channel = StatusChannel::new(
            ArbitraryGenerator::new().generate(),
            ArbitraryGenerator::new().generate(),
            None,
        );
        let params = Arc::new(gen_params());
        let ctx = WriterContext::new(params, cfg, addr, client, status_channel);
        Arc::new(ctx)
    }
}
