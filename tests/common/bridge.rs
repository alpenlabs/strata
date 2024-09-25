//! This module contains utilities for integration tests related to the bridge.

use std::{collections::BTreeMap, ops::Not, sync::Arc, time::Duration};

use alpen_express_common::logging;
use alpen_express_primitives::{
    bridge::{Musig2PartialSig, Musig2PubNonce, OperatorIdx, OperatorPartialSig, PublickeyTable},
    buf::{Buf20, Buf32},
    l1::{BitcoinAddress, XOnlyPk},
};
use alpen_express_rocksdb::{bridge::db::BridgeTxRocksDb, test_utils::get_rocksdb_tmp_instance};
use alpen_test_utils::bridge::generate_keypairs;
use anyhow::Context;
use bitcoin::{
    key::Keypair,
    secp256k1::{PublicKey, SecretKey, XOnlyPublicKey, SECP256K1},
    taproot::{LeafVersion, TaprootBuilder},
    Address, Amount, Network, OutPoint, TapNodeHash, Transaction, Txid,
};
use bitcoind::{
    bitcoincore_rpc::{
        json::{AddressType, ListUnspentResultEntry, SignRawTransactionResult},
        RpcApi,
    },
    BitcoinD, Conf,
};
use express_bridge_sig_manager::prelude::SignatureManager;
use express_bridge_tx_builder::{
    prelude::{
        create_tx, create_tx_ins, create_tx_outs, get_aggregated_pubkey, metadata_script,
        n_of_n_script, CooperativeWithdrawalInfo, DepositInfo, TxBuildContext, BRIDGE_DENOMINATION,
        UNSPENDABLE_INTERNAL_KEY,
    },
    TxKind,
};
use express_storage::ops;
use threadpool::ThreadPool;
use tokio::{
    sync::{broadcast, Mutex},
    time::{error::Elapsed, timeout},
};
use tracing::{debug, event, span, trace, warn, Level};

/// Transaction fee to confirm Deposit Transaction
///
/// This value must be greater than 179 based on the current config in the `bitcoind` instance.
pub(crate) const DT_FEE: Amount = Amount::from_sat(1_500); // should be more than enough
/// Minimum confirmations required for miner rewards to become spendable.
pub(crate) const MIN_MINER_REWARD_CONFS: u64 = 101;

#[derive(Debug)]
pub(crate) struct BridgeFederation {
    pub(crate) operators: Vec<Operator>,
    pub(crate) pubkey_table: PublickeyTable,
}

impl BridgeFederation {
    pub(crate) async fn new(num_operators: usize, bitcoind: Arc<Mutex<BitcoinD>>) -> Self {
        let (pks, sks) = generate_keypairs(num_operators);

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
                    "operator",
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

    #[allow(dead_code)] // this is used in the `cooperative-bridge-flow` and nowhere else.
                        // See docstring on this module.
                        // HACK: to get a copy of the federation that we can mutate inside a tokio thread as `Operator`
                        // is not `Clone` due to broadcast receive channel.
    pub(crate) async fn duplicate(&self, duplicate_id: &str) -> Self {
        let pubkey_table = self.pubkey_table.clone();
        let num_operators = pubkey_table.0.keys().len();

        let queue_size = num_operators * 2; // buffer for nonces and signatures (overkill)
        let (msg_tx, _msg_recv) = broadcast::channel::<Message>(queue_size);

        let mut operators = Vec::with_capacity(num_operators);
        for operator in self.operators.iter() {
            let msg_sender = msg_tx.clone();
            let msg_receiver = msg_tx.subscribe();

            operators.push(
                Operator::new(
                    duplicate_id,
                    operator.secret_key,
                    pubkey_table.clone(),
                    operator.agent.bitcoind.clone(),
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
    // This is used in `bridge-in-flow` but not in `cooperative-bridge-out-flow`
    // See docstring on this module.
    #[allow(unused)]
    Deposit(DepositInfo),

    // This is used in `cooperative-bridge-out-flow` but not in `bridge-in-flow`
    // See docstring on this module.
    #[allow(unused)]
    Withdrawal(CooperativeWithdrawalInfo),
}

/// The messages that need to be share among operators.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    Nonce(Txid, Musig2PubNonce, OperatorIdx),
    Signature(Txid, Musig2PartialSig, OperatorIdx),
}

/// An operator is an agent that is a member of the bridge federation capable of processing deposits
/// and withdrawals.
#[derive(Debug)]
pub(crate) struct Operator {
    /// The agent that manages the operator's keys and the bitcoind client.
    pub(crate) agent: Agent,

    /// The secret key of this operator.
    secret_key: SecretKey,

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
        id: &str,
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

        let id = format!("{}-{}", id, index);
        let agent = Agent::new(&id, bitcoind).await;
        let keypair = Keypair::from_secret_key(SECP256K1, &secret_key);
        let sig_manager = setup_sig_manager(index, keypair);

        let tx_builder = TxBuildContext::new(Network::Regtest, pubkey_table.clone(), index);

        Self {
            agent,
            secret_key,
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
                // deposit involves just creating an aggregated pubkey that can be verified with
                // `OP_CHECKSIG`
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

                let fully_signed_transaction = self.sig_manager.finalize_transaction(&txid).await;

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
                let tx_signing_data = withdrawal_info.construct_signing_data(&self.tx_builder);
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

                let fully_signed_transaction = self.sig_manager.finalize_transaction(&txid).await;

                assert!(
                    fully_signed_transaction.is_ok(),
                    "should be able to produce a fully signed transaction but got: {:?}",
                    fully_signed_transaction.unwrap_err()
                );

                let signed_tx = fully_signed_transaction.unwrap();

                event!(
                    Level::WARN,
                    action = "broadcasting signed withdrawal transaction",
                    operator_idx = %self.index
                );
                let txid = self.agent.broadcast_signed_tx(&signed_tx).await;

                event!(Level::INFO, event = "broadcasted withdrawal transaction", txid = %txid, operator_idx = %self.index);
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
            result.is_ok(),
            "should be able to add own signature but got: {:?}",
            result.unwrap_err()
        );
        assert!(
            result.is_ok_and(|is_complete| is_complete.not()),
            "should be able to add own signature but should not complete the collection"
        );

        event!(Level::INFO, event = "added own signature", operator_idx=%self.index);

        let own_sig = self.sig_manager.get_own_partial_sig(txid).await;
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

                    let signature_info = OperatorPartialSig::new(partial_sig, sender_idx);
                    let is_complete = self
                        .sig_manager
                        .add_partial_sig(&txid, signature_info).await;

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

    #[allow(unused)] // this is used in `bridge-in-flow` but not in `cooperative-bridge-out-flow`
                     // See docstring on this module.
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

    #[allow(unused)] // This is used in `cooperative-bridge-out-flow` but not in `bridge-in-flow`
                     // See docstring on this module.
    pub(crate) fn pubkey(&self) -> XOnlyPk {
        let script_pubkey = self.address.script_pubkey();
        let script_pubkey = &script_pubkey.as_bytes()[2..34];

        let x_only_pk = XOnlyPublicKey::from_slice(script_pubkey).unwrap();

        let x_only_pk = Buf32(x_only_pk.serialize().into());

        XOnlyPk::new(x_only_pk)
    }

    /// Mines [`Self::MIN_BLOCKS_TILL_SPENDABLE`] + `num_blocks` to this user's address.
    pub(crate) async fn mine_blocks(&self, num_blocks: u64) -> Amount {
        let bitcoind = self.bitcoind.lock().await;

        let _ = bitcoind
            .client
            .generate_to_address(num_blocks, &self.address)
            .context("could not mine blocks")
            .map(|hashes| hashes.len());

        // confirm balance
        bitcoind
            .client
            .get_balance(None, None)
            .expect("should be able to extract balance")
    }

    pub(crate) async fn get_unspent_utxos(&self) -> Vec<ListUnspentResultEntry> {
        let bitcoind = self.bitcoind.lock().await;

        bitcoind
            .client
            .list_unspent(None, None, Some(&[&self.address]), Some(false), None)
            .expect("should get unspent transactions")
    }

    pub(crate) async fn select_utxo(
        &self,
        target_amount: Amount,
    ) -> Option<(Address, OutPoint, Amount)> {
        let unspent_utxos = self.get_unspent_utxos().await;

        let bitcoind = self.bitcoind.lock().await;

        let change_address = bitcoind
            .client
            .get_raw_change_address(Some(AddressType::Bech32m))
            .expect("should get change address")
            .require_network(Network::Regtest)
            .expect("address should be valid for Regtest");

        for entry in unspent_utxos {
            trace!(%entry.amount, %entry.txid, %entry.vout, %entry.confirmations, "checking unspent utxos");
            if entry.amount > target_amount + DT_FEE {
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

        let high_priority_fee_rate = Amount::from_sat(8); // high priority

        let total_in_value =
            tx.input
                .iter()
                .fold(Amount::from_int_btc(0), |total_in_value, txin| {
                    let prev_vout = txin.previous_output.vout;
                    let prev_txid = txin.previous_output.txid;
                    let prev_tx = bitcoind
                        .client
                        .get_raw_transaction(&prev_txid, None)
                        .expect("previous transaction should exist");

                    let prev_value = prev_tx.output[prev_vout as usize].value;

                    total_in_value + prev_value
                });

        let total_out_value: Amount = tx.output.iter().map(|txout| txout.value).sum();
        let actual_fees = total_in_value - total_out_value;

        let estimated_fees: Amount = high_priority_fee_rate * tx.weight().to_vbytes_ceil();

        warn!(
            ?high_priority_fee_rate,
            ?estimated_fees,
            ?actual_fees,
            "Fee calculation for the transaction"
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
    conf.args = vec![
        "-regtest",
        "-fallbackfee=0.00001",
        "-maxtxfee=0.0001",
        "-txindex=1",
    ];

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

#[allow(dead_code)] // This not used in the `cooperative-bridge-out-flow`.
                    // See docstring on this module.
pub(crate) async fn perform_user_actions(
    user: &User,
    federation_pubkey_table: PublickeyTable,
) -> (Txid, TapNodeHash, Address, Vec<u8>) {
    let span = span!(Level::INFO, "user actions");
    let _guard = span.enter();

    event!(Level::INFO, action = "sending funds to user's address");
    let balance = user.agent().mine_blocks(MIN_MINER_REWARD_CONFS).await;
    event!(Level::INFO, user_balance = %balance);

    assert!(
        balance.gt(&BRIDGE_DENOMINATION.into()),
        "user balance must be greater than the bridge denomination, got: {}, expected > {}",
        balance,
        BRIDGE_DENOMINATION
    );
    event!(Level::INFO, action = "getting available utxos");

    let (change_address, outpoint, amount) = user
        .agent()
        .select_utxo(BRIDGE_DENOMINATION.into())
        .await
        .expect("should get utxo with enough amount");
    event!(Level::INFO, event = "got change address and outpoint to use", change_address = %change_address, outpoint = %outpoint, amount = %amount);

    let (drt, take_back_leaf_hash, taproot_addr, el_address) = create_drt(
        outpoint,
        federation_pubkey_table,
        *UNSPENDABLE_INTERNAL_KEY,
        change_address,
        amount,
    );
    event!(Level::TRACE, event = "created DRT", drt = ?drt);

    event!(Level::INFO, action = "signing DRT with wallet");
    let signed_tx_result = user.agent().sign_raw_tx(&drt).await;
    assert!(signed_tx_result.complete, "tx should be fully signed");

    let signed_tx = signed_tx_result
        .transaction()
        .expect("should be able to get fully signed transaction");

    event!(Level::INFO, action = "broadcasting signed DRT");
    let txid = user.agent().broadcast_signed_tx(&signed_tx).await;
    event!(Level::INFO, event = "broadcasted signed DRT", txid = %txid);

    (txid, take_back_leaf_hash, taproot_addr, el_address)
}

pub(crate) fn create_drt(
    outpoint: OutPoint,
    pubkeys: PublickeyTable,
    internal_key: XOnlyPublicKey,
    change_address: Address,
    total_amt: Amount,
) -> (Transaction, TapNodeHash, Address, Vec<u8>) {
    let input = create_tx_ins([outpoint]);

    let (drt_addr, take_back_leaf_hash, el_address) =
        create_drt_taproot_output(pubkeys, internal_key);

    let net_bridge_in_amount = Amount::from(BRIDGE_DENOMINATION) + DT_FEE;

    let drt_pubkey = drt_addr.script_pubkey();
    let change_pubkey = change_address.script_pubkey();

    let tx_fees = drt_pubkey.minimal_non_dust() + change_pubkey.minimal_non_dust();

    let output = create_tx_outs([
        (drt_pubkey, net_bridge_in_amount),
        (change_pubkey, total_amt - net_bridge_in_amount - tx_fees),
    ]);

    (
        create_tx(input, output),
        take_back_leaf_hash,
        drt_addr,
        el_address,
    )
}

pub(crate) fn create_drt_taproot_output(
    pubkeys: PublickeyTable,
    internal_key: XOnlyPublicKey,
) -> (Address, TapNodeHash, Vec<u8>) {
    let aggregated_pubkey = get_aggregated_pubkey(pubkeys);
    let n_of_n_spend_script = n_of_n_script(&aggregated_pubkey);

    // in actual DRT, this will be the take-back leaf.
    // for testing, this could be any script as we only care about its hash.
    let el_address = Buf20::default().0 .0;
    let op_return_script = metadata_script(&el_address[..].try_into().unwrap());
    let op_return_script_hash = TapNodeHash::from_script(&op_return_script, LeafVersion::TapScript);

    let taproot_builder = TaprootBuilder::new()
        .add_leaf(1, n_of_n_spend_script.clone())
        .unwrap()
        .add_leaf(1, op_return_script)
        .unwrap();

    let spend_info = taproot_builder.finalize(SECP256K1, internal_key).unwrap();

    (
        Address::p2tr(
            SECP256K1,
            internal_key,
            spend_info.merkle_root(),
            Network::Regtest,
        ),
        op_return_script_hash,
        el_address.to_vec(),
    )
}

#[allow(dead_code)] // This is not used in the `cooperative-bridge-out-flow`.
                    // See docstring on this module.
pub(crate) fn perform_rollup_actions(
    txid: Txid,
    take_back_leaf_hash: TapNodeHash,
    original_taproot_addr: Address,
    el_address: &[u8; 20],
) -> DepositInfo {
    let span = span!(Level::INFO, "rollup actions");
    let _guard = span.enter();

    let deposit_request_outpoint = OutPoint { txid, vout: 0 };
    let total_amount: Amount = Amount::from(BRIDGE_DENOMINATION) + DT_FEE;
    let original_taproot_addr =
        BitcoinAddress::parse(&original_taproot_addr.to_string(), Network::Regtest)
            .expect("address should be valid for network");

    event!(Level::INFO, action = "creating deposit info");
    DepositInfo::new(
        deposit_request_outpoint,
        el_address.to_vec(),
        total_amount,
        take_back_leaf_hash,
        original_taproot_addr,
    )
}
