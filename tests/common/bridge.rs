//! This module contains utilities for integratation tests related to the bridge.

use std::{collections::BTreeMap, ops::Not, sync::Arc, time::Duration};

use alpen_express_common::logging;
use alpen_express_primitives::bridge::{
    Musig2PubNonce, OperatorIdx, OperatorPartialSig, PublickeyTable, SignatureInfo,
};
use alpen_express_rocksdb::{bridge::db::BridgeTxRocksDb, test_utils::get_rocksdb_tmp_instance};
use alpen_test_utils::bridge::generate_keypairs;
use anyhow::Context;
use bitcoin::{
    key::Keypair,
    secp256k1::{PublicKey, SecretKey, SECP256K1},
    Address, Amount, Network, OutPoint, Transaction, Txid,
};
use bitcoind::{
    bitcoincore_rpc::{
        json::{AddressType, SignRawTransactionResult},
        RpcApi,
    },
    BitcoinD, Conf,
};
use express_bridge_sig_manager::prelude::SignatureManager;
use express_bridge_tx_builder::{
    prelude::{CooperativeWithdrawalInfo, DepositInfo, TxBuildContext},
    TxKind,
};
use express_storage::ops;
use threadpool::ThreadPool;
use tokio::{
    sync::{broadcast, Mutex},
    time::{error::Elapsed, timeout},
};
use tracing::{debug, event, span, trace, Level};

pub(crate) const MIN_FEE: Amount = Amount::from_sat(10000); // some random value; nothing special

pub(crate) struct BridgeFederation {
    pub(crate) operators: Vec<Operator>,
    pub(crate) pubkey_table: PublickeyTable,
}

impl BridgeFederation {
    pub(crate) async fn new(num_operators: usize, bitcoind: Arc<Mutex<BitcoinD>>) -> Self {
        let (pks, sks) = generate_keypairs(SECP256K1, num_operators);

        let mut pubkey_table = BTreeMap::new();
        for (operator_idx, pk) in pks.iter().enumerate() {
            pubkey_table.insert(operator_idx as OperatorIdx, *pk);
        }

        let pubkey_table: PublickeyTable = pubkey_table.into();

        let queue_size = num_operators * 2; // buffer for nonces and signatures (overkill)
        let (msg_tx, _msg_recv) = broadcast::channel::<Message>(queue_size);

        let mut operators = Vec::with_capacity(num_operators);
        for sk in sks {
            let msg_sender = msg_tx.clone();
            let msg_receiver = msg_tx.subscribe();

            operators.push(
                Operator::new(
                    sk,
                    pubkey_table.clone(),
                    bitcoind.clone(),
                    msg_sender,
                    msg_receiver,
                )
                .await,
            )
        }

        Self {
            operators,
            pubkey_table,
        }
    }
}

/// The bridge duties that can be extracted from the chain state in the rollup.
#[derive(Debug, Clone)]
pub(crate) enum BridgeDuty {
    Deposit(DepositInfo),
    #[allow(dead_code)] // to be handled later
    Withdrawal(CooperativeWithdrawalInfo),
}

/// The messages that need to be share among operators.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    Nonce(Txid, Musig2PubNonce, OperatorIdx),
    Signature(Txid, OperatorPartialSig, OperatorIdx),
}

/// An operator is an agent that is a member of the bridge federation capable of processing deposits
/// and withdrawals.
pub(crate) struct Operator {
    /// The agent that manages the operator's keys and the bitcoind client.
    pub(crate) agent: Agent,

    /// The table of pubkeys for the federation.
    pub(crate) pubkey_table: PublickeyTable,

    /// The index for this operator.
    pub(crate) index: OperatorIdx,

    /// The transaction builder for this instance.
    pub(crate) tx_builder: TxBuildContext,

    /// The signature manager for this operator.
    pub(crate) sig_manager: SignatureManager,

    /// The message sender for this operator.
    pub(crate) msg_sender: broadcast::Sender<Message>,

    /// The message receiver for this operator.
    pub(crate) msg_receiver: broadcast::Receiver<Message>,
}

impl Operator {
    // too many arguments here, perhaps convert this to a builder?
    pub(crate) async fn new(
        secret_key: SecretKey,
        pubkey_table: PublickeyTable,
        bitcoind: Arc<Mutex<BitcoinD>>,
        msg_sender: broadcast::Sender<Message>,
        msg_receiver: broadcast::Receiver<Message>,
    ) -> Self {
        let public_key = PublicKey::from_secret_key(SECP256K1, &secret_key);

        let index = *pubkey_table
            .0
            .iter()
            .find_map(|(idx, pk)| if *pk == public_key { Some(idx) } else { None })
            .expect("pubkey should be part of the pubkey table");

        let agent = Agent::new(&index.to_string(), bitcoind).await;
        let keypair = Keypair::from_secret_key(SECP256K1, &secret_key);
        let sig_manager = setup_sig_manager(index, keypair);

        let tx_builder = TxBuildContext::new(pubkey_table.clone(), Network::Regtest);

        Self {
            agent,
            pubkey_table,
            index,
            tx_builder,
            sig_manager,
            msg_sender,
            msg_receiver,
        }
    }

    pub(crate) async fn process_duty(&mut self, duty: BridgeDuty) {
        match duty {
            BridgeDuty::Deposit(deposit_info) => {
                event!(Level::TRACE, action = "starting to create tx signing data", deposit_info = ?deposit_info, operator_idx=%self.index);

                let tx_signing_data = deposit_info.construct_signing_data(&self.tx_builder);
                assert!(
                    tx_signing_data.is_ok(),
                    "should be able to construct the signing data but got error: {:?}",
                    tx_signing_data.unwrap_err()
                );

                let tx_signing_data = tx_signing_data.unwrap();

                let txid = self
                    .sig_manager
                    .add_tx_state(tx_signing_data, self.pubkey_table.clone())
                    .await;
                assert!(
                    txid.is_ok(),
                    "should be able to add tx state to the sig manager but got: {:?}",
                    txid.unwrap_err()
                );

                let txid = txid.unwrap();
                event!(Level::INFO, event = "added new tx state to sig manager db", txid = %txid, operator_idx=%self.index);

                let timeout_duration = Duration::from_secs(60); // should be more than enough

                let receive_nonces = self.aggregate_nonces(&txid, timeout_duration).await;
                assert!(
                    receive_nonces.is_ok(),
                    "timeout while trying to receive nonces"
                );

                event!(Level::INFO, event = "all nonces collected", operator_idx=%self.index);

                let receive_signatures = self.agggregate_signatures(&txid, timeout_duration).await;
                assert!(
                    receive_signatures.is_ok(),
                    "timeout while trying to collect signatures"
                );

                event!(Level::INFO, event = "signature collection complete", operator_idx=%self.index);

                let fully_signed_transaction =
                    self.sig_manager.get_fully_signed_transaction(&txid).await;

                assert!(
                    fully_signed_transaction.is_ok(),
                    "should be able to produce a fully signed transaction but got: {:?}",
                    fully_signed_transaction.unwrap_err()
                );

                let signed_tx = fully_signed_transaction.unwrap();

                event!(
                    Level::WARN,
                    action = "broadcasting signed deposit transaction",
                    operator_idx = %self.index
                );
                let txid = self.agent.broadcast_signed_tx(&signed_tx).await;

                event!(Level::INFO, event = "broadcasted DT", txid = %txid, operator_idx = %self.index);
            }
            BridgeDuty::Withdrawal(withdrawal_info) => {
                event!(Level::TRACE, action = "starting to create tx signing data", withdrawal_info = ?withdrawal_info, operator_idx = %self.index);
                todo!()
            }
        }
    }

    async fn aggregate_nonces(
        &mut self,
        txid: &Txid,
        timeout_duration: Duration,
    ) -> Result<(), Elapsed> {
        let nonce = self.sig_manager.get_own_nonce(txid).await;
        assert!(
            nonce.is_ok(),
            "should be able to retrieve own nonce but got: {:?}",
            nonce.unwrap_err()
        );

        event!(Level::INFO, action = "broadcasting nonce", operator_idx=%self.index);
        self.msg_sender
            .send(Message::Nonce(*txid, nonce.unwrap(), self.index))
            .expect("should be able to send nonce");

        event!(Level::INFO, action = "starting to process nonces");

        timeout(timeout_duration, async {
            loop {
                if let Ok(Message::Nonce(txid, pub_nonce, sender_idx)) =
                    self.msg_receiver.recv().await
                {
                    if sender_idx == self.index {
                        continue;
                    }

                    event!(Level::DEBUG, event = "received nonce", sender_idx = %sender_idx, operator_idx = %self.index);

                    let is_complete = self
                        .sig_manager
                        .add_nonce(&txid, sender_idx, &pub_nonce)
                        .await;

                    assert!(
                        is_complete.is_ok(),
                        "expected add_nonce to work but got error: {}",
                        is_complete.unwrap_err()
                    );

                    if is_complete.unwrap() {
                        let tx_state = self.sig_manager.get_tx_state(&txid).await.expect("should be able to access state");
                        let agg_nonce = self.sig_manager.get_aggregated_nonce(&tx_state);

                        event!(Level::DEBUG, event = "nonce aggregation complete", nonce = ?agg_nonce, operator_idx = %self.index);
                        break;
                    }
                }
            }
        })
        .await
    }

    async fn agggregate_signatures(
        &mut self,
        txid: &Txid,
        timeout_duration: Duration,
    ) -> Result<(), Elapsed> {
        let result = self.sig_manager.add_own_partial_sig(txid).await;
        assert!(
            result.is_ok_and(|is_complete| is_complete.not()),
            "should be able to add own signature but should not complete the collection"
        );

        event!(Level::INFO, event = "added own signature", operator_idx=%self.index);

        let input_index = 0; // for now, this is always 0
        let own_sig = self
            .sig_manager
            .get_own_partial_sig(txid, input_index)
            .await;
        assert!(
            own_sig.is_ok(),
            "should be able to get one's own signature: ({})",
            self.index
        );
        let own_sig = own_sig.unwrap();

        assert!(
            own_sig.is_some(),
            "one's own signature must be defined at this point ({})",
            self.index
        );
        let own_sig = own_sig.unwrap();

        let result = self
            .msg_sender
            .send(Message::Signature(*txid, own_sig, self.index));
        assert!(
            result.is_ok(),
            "should be able to send out partial signature ({})",
            self.index
        );

        timeout(timeout_duration, async {
            loop {
                if let Ok(Message::Signature(txid, partial_sig, sender_idx)) =
                    self.msg_receiver.recv().await
                {
                    if sender_idx == self.index {
                        continue;
                    }

                    event!(Level::DEBUG, event = "received signature", sender_idx = %sender_idx, operator_idx = %self.index);

                    let signature_info = SignatureInfo::new(partial_sig, sender_idx);
                    let input_index = 0; // always going to be one input for now
                    let is_complete = self
                        .sig_manager
                        .add_partial_sig(&txid, signature_info, input_index)
                        .await;

                    assert!(
                        is_complete.is_ok(),
                        "should be able to add signature but got: {:?}",
                        is_complete.unwrap()
                    );

                    if is_complete.unwrap() {
                        event!(Level::INFO, event = "signature aggregation complete", operator_idx = %self.index );

                        break;
                    }
                }
            }
        })
        .await
    }
}

// A user is simply an agent.
#[derive(Debug, Clone)]
pub(crate) struct User(Agent);

impl User {
    pub(crate) async fn new(id: &str, bitcoind: Arc<Mutex<BitcoinD>>) -> Self {
        Self(Agent::new(id, bitcoind).await)
    }

    pub(crate) fn address(&self) -> &Address {
        &self.0.address
    }

    pub(crate) fn agent(&self) -> &Agent {
        &self.0
    }
}

/// An agent is any entity that is capable of performing some actions on bitcoin with the help of a
/// single key.
#[derive(Debug, Clone)]
pub(crate) struct Agent {
    /// The address for this agent.
    pub(crate) address: Address,

    /// The bitcoin instance for the agent.
    bitcoind: Arc<Mutex<BitcoinD>>,
}

impl Agent {
    const MIN_BLOCKS_TILL_SPENDABLE: u64 = 100;

    pub(crate) async fn new(id: &str, bitcoind: Arc<Mutex<BitcoinD>>) -> Self {
        let address = {
            let bitcoind = bitcoind.lock().await;

            bitcoind
                .client
                .create_wallet(id, None, None, None, None)
                .expect("should create a wallet");

            bitcoind
                .client
                .get_new_address(Some(id), Some(AddressType::Bech32m))
                .expect("address should be generated")
                .require_network(Network::Regtest)
                .expect("address should be valid")
        };

        Self { address, bitcoind }
    }

    /// Mines [`Self::MIN_BLOCKS_TILL_SPENDABLE`] + `num_blocks` to this user's address.
    pub(crate) async fn mine_blocks(&self, num_blocks: u64) -> Amount {
        let bitcoind = self.bitcoind.lock().await;

        let _ = bitcoind
            .client
            .generate_to_address(Self::MIN_BLOCKS_TILL_SPENDABLE + num_blocks, &self.address)
            .context("could not mine blocks")
            .map(|hashes| hashes.len());

        // confirm balance
        bitcoind
            .client
            .get_balance(Some(Self::MIN_BLOCKS_TILL_SPENDABLE as usize), None)
            .expect("should be able to extract balance")
    }

    pub(crate) async fn select_utxo(
        &self,
        target_amount: Amount,
    ) -> Option<(Address, OutPoint, Amount)> {
        let bitcoind = self.bitcoind.lock().await;

        let unspent_utxos = bitcoind
            .client
            .list_unspent(None, None, Some(&[&self.address]), Some(false), None)
            .expect("should get unspent transactions");
        let change_address = bitcoind
            .client
            .get_raw_change_address(Some(AddressType::Bech32m))
            .expect("should get change address")
            .require_network(Network::Regtest)
            .expect("address should be valid for Regtest");

        for entry in unspent_utxos {
            trace!(%entry.amount, %entry.txid, %entry.vout, %entry.confirmations, "checking unspent utxos");
            if entry.amount > target_amount + MIN_FEE {
                return Some((
                    change_address,
                    OutPoint {
                        txid: entry.txid,
                        vout: entry.vout,
                    },
                    entry.amount,
                ));
            }
        }

        None
    }

    pub(crate) async fn sign_raw_tx(&self, tx: &Transaction) -> SignRawTransactionResult {
        let bitcoind = self.bitcoind.lock().await;
        let result = bitcoind
            .client
            .sign_raw_transaction_with_wallet(tx, None, None);

        assert!(
            result.is_ok(),
            "should be able to sign the raw transaction but got: {}",
            result.unwrap_err()
        );

        result.unwrap()
    }

    pub(crate) async fn broadcast_signed_tx(&self, tx: &Transaction) -> Txid {
        debug!(?tx, "broadcasting transaction");
        let bitcoind = self.bitcoind.lock().await;

        let result = bitcoind.client.test_mempool_accept(&[tx]);
        assert!(
            result.is_ok(),
            "should pass mempool test but got err: {:?}",
            result.unwrap_err()
        );

        let result = bitcoind.client.send_raw_transaction(tx);

        assert!(
            result.is_ok(),
            "should be able to send raw transaction but got: {:?} for {:?}",
            result.err(),
            tx
        );

        result.unwrap()
    }
}

pub(crate) async fn setup(num_operators: usize) -> (Arc<Mutex<BitcoinD>>, BridgeFederation) {
    logging::init();

    let span = span!(Level::INFO, "setup federation");
    let _guard = span.enter();

    let mut conf = Conf::default();
    conf.args = vec!["-regtest", "-fallbackfee=0.00001", "-maxtxfee=1.1"];

    // Uncomment the following line to view the stdout from `bitcoind`
    // conf.view_stdout = true;

    event!(Level::INFO, action = "starting bitcoind", conf = ?conf);
    let bitcoind = BitcoinD::from_downloaded_with_conf(&conf).expect("bitcoind client must start");
    let bitcoind = Arc::new(Mutex::new(bitcoind));

    event!(Level::INFO, action = "setting up a bridge federation", num_operator = %num_operators);

    let federation = BridgeFederation::new(num_operators, bitcoind.clone()).await;
    (bitcoind, federation)
}

pub(crate) fn setup_sig_manager(index: OperatorIdx, keypair: Keypair) -> SignatureManager {
    let span = span!(Level::INFO, "setup sig manager", operator_idx = %index);
    let _guard = span.enter();

    event!(Level::INFO, action = "setting up tmp rocksdb instance");
    let (bridge_tx_db, config) =
        get_rocksdb_tmp_instance().expect("should create a tmp rocksdb instance");

    let bridge_tx_db = BridgeTxRocksDb::new(bridge_tx_db, config);

    let bridge_tx_db_ctx = ops::bridge::Context::new(Arc::new(bridge_tx_db));

    let num_threads = 1;
    let thread_pool = ThreadPool::new(num_threads);
    let db_ops = bridge_tx_db_ctx.into_ops(thread_pool);

    event!(Level::INFO, event = "database handler initialized");

    SignatureManager::new(Arc::new(db_ops), index, keypair)
}
