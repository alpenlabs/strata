//! Defines the main loop for the bridge-client in operator mode.

use std::{fs, path::PathBuf, sync::Arc};

use alpen_express_btcio::rpc::{
    traits::{Reader, Signer},
    BitcoinClient,
};
use alpen_express_primitives::bridge::OperatorIdx;
use alpen_express_rocksdb::{
    bridge::db::{BridgeDutyRocksDb, BridgeTxRocksDb},
    DbOpsConfig, ROCKSDB_NAME, STORE_COLUMN_FAMILIES,
};
use alpen_express_rpc_api::AlpenApiClient;
use alpen_express_state::bridge_duties::{BridgeDuty, BridgeDutyStatus};
use bitcoin::{secp256k1::SECP256K1, Txid};
use directories::ProjectDirs;
use express_bridge_exec::handler::{aggregate_sig, sign_tx};
use express_bridge_sig_manager::manager::SignatureManager;
use express_bridge_tx_builder::{context::TxBuildContext, TxKind};
use express_storage::ops::{
    bridge::Context as TxContext,
    bridge_duty::{BridgeDutyOps, Context as DutyContext},
};
use jsonrpsee::{core::client::async_client::Client, ws_client::WsClientBuilder};
use rockbound::{rocksdb, OptimisticTransactionDB};
use threadpool::ThreadPool;
use tokio::{
    spawn,
    time::{interval, Duration},
    try_join,
};

#[allow(unused)] // FIXME: remove once these imports are used
use crate::rpc_server::{self, BridgeRpc};
use crate::{args::Cli, constants::ROCKSDB_RETRY_COUNT};

/// Bootstraps the bridge client in Operator mode by hooking up all the required auxiliary services
/// including database, rpc server, etc. Threadpool and logging need to be initialized at the call
/// site (main function) itself.
pub(crate) async fn bootstrap() -> anyhow::Result<()> {
    let cli_args: Cli = argh::from_env();

    // Parse the data_dir
    let data_dir = cli_args.data_dir.map(PathBuf::from);

    // Initialize a rocksdb instance with the required column families.
    let rbdb = open_rocksdb_database(data_dir)?;
    let retry_count = cli_args.retry_count.unwrap_or(ROCKSDB_RETRY_COUNT);
    let ops_config = DbOpsConfig::new(retry_count);

    // Arc'up the BridgeDutyOps
    let bridge_duty_db = BridgeDutyRocksDb::new(rbdb.clone(), ops_config);
    let bridge_duty_db_ctx = DutyContext::new(Arc::new(bridge_duty_db));
    let bridge_duty_db_pool = ThreadPool::new(1);
    let bridge_duty_db_ops = Arc::new(bridge_duty_db_ctx.into_ops(bridge_duty_db_pool));

    // Arc'up the L1/L2 clients
    let l1_rpc_client = Arc::new(
        BitcoinClient::new(cli_args.btc_url, cli_args.btc_user, cli_args.btc_pass)
            .expect("error creating the bitcoin client"),
    );
    let sync_client: Client = WsClientBuilder::default()
        .build(cli_args.rollup_url)
        .await
        .expect("failed to connect to the rollup RPC server");
    let l2_rpc_client = Arc::new(sync_client);

    // Get our pubkey from the bitcoin wallet
    let operator_pubkeys = l2_rpc_client.get_active_operator_chain_pubkey_set().await?;
    let keypair = l1_rpc_client
        .get_xpriv()
        .await?
        .expect("could not get a valid xpriv from the bitcoin wallet")
        .to_keypair(SECP256K1);
    let pubkey = keypair.public_key();
    let operator_index: OperatorIdx = operator_pubkeys
        .0
        .iter()
        .find_map(|(id, pk)| if pk == &pubkey { Some(*id) } else { None })
        .expect("could not find this operator's pubkey in the rollup pubkey table");

    // Arc'up the SignatureManager
    let bridge_tx_db = BridgeTxRocksDb::new(rbdb, ops_config);
    let bridge_tx_db_ctx = TxContext::new(Arc::new(bridge_tx_db));
    let bridge_tx_db_pool = ThreadPool::new(1);
    let bridge_tx_db_ops = Arc::new(bridge_tx_db_ctx.into_ops(bridge_tx_db_pool));
    let sig_manager = Arc::new(SignatureManager::new(
        bridge_tx_db_ops,
        operator_index,
        keypair,
    ));

    // Arc'up the TxBuildContext
    let network = l1_rpc_client.network().await?;
    let tx_context = Arc::new(TxBuildContext::new(
        network,
        operator_pubkeys,
        operator_index,
    ));

    // Default: rollup block time
    let duty_pooling_interval = cli_args.duty_interval.map_or(
        Duration::from_millis(
            l2_rpc_client
                .block_time()
                .await
                .expect("could not get default block time from rollup RPC client"),
        ),
        Duration::from_millis,
    );

    // Spawn poll duties task
    let l1_rpc_client_duty = Arc::clone(&l1_rpc_client);
    let l2_rpc_client_duty = Arc::clone(&l2_rpc_client);
    let duty_task = spawn(async move {
        let mut interval = interval(duty_pooling_interval);
        loop {
            interval.tick().await;
            match poll_duties(
                operator_index,
                bridge_duty_db_ops.clone(),
                tx_context.clone(),
                l2_rpc_client_duty.clone(),
            )
            .await
            {
                Ok(duties) => {
                    for (txid, duty) in duties.iter() {
                        let tx_state = sig_manager
                            .get_tx_state(txid)
                            .await
                            .expect("could not get tx state for: {txid}");

                        // if fully signed then skip it
                        if tx_state.is_fully_signed() {
                            continue;
                        }

                        // otherwise check the duty type
                        // TODO: refactor this whole match to take a single <dyn TxKind>
                        match duty {
                            BridgeDuty::SignDeposit(deposit) => {
                                // if does not have this operator's signature then sign it
                                if tx_state.collected_sigs().get(&operator_index).is_none() {
                                    sign_tx(
                                        deposit,
                                        &(*l1_rpc_client_duty),
                                        &(*l2_rpc_client_duty),
                                        &sig_manager,
                                        &tx_context,
                                    )
                                    .await
                                    .expect("could not sign transaction: {txid}");
                                }
                                // if has this operator's sig it needs to be aggregated and wait for
                                else if tx_state.collected_sigs().get(&operator_index).is_some() {
                                    aggregate_sig(
                                        txid,
                                        &(*l1_rpc_client_duty),
                                        &(*l2_rpc_client_duty),
                                        &sig_manager,
                                        &tx_context,
                                    )
                                    .await
                                    .expect(
                                        "could not aggregate signatures for transaction: {txid}",
                                    );
                                }
                                // emit a warning if in another state
                                else {
                                    eprintln!("could not fulfill duty: {txid}");
                                }
                            }
                            BridgeDuty::FulfillWithdrawal(withdrawal) => {
                                // if does not have this operator's signature then sign it
                                if tx_state.collected_sigs().get(&operator_index).is_none() {
                                    sign_tx(
                                        withdrawal,
                                        &(*l1_rpc_client_duty),
                                        &(*l2_rpc_client_duty),
                                        &sig_manager,
                                        &tx_context,
                                    )
                                    .await
                                    .expect("could not sign transaction: {txid}");
                                }
                                // if has this operator's sig it needs to be aggregated and wait for
                                else if tx_state.collected_sigs().get(&operator_index).is_some() {
                                    aggregate_sig(
                                        txid,
                                        &(*l1_rpc_client_duty),
                                        &(*l2_rpc_client_duty),
                                        &sig_manager,
                                        &tx_context,
                                    )
                                    .await
                                    .expect(
                                        "could not aggregate signatures for transaction: {txid}",
                                    );
                                }
                                // emit a warning if in another state
                                else {
                                    eprintln!("could not fulfill duty: {txid}");
                                }
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Failed to poll bridge duties: {:?}", e),
            }
        }
    });

    // Spawn RPC server
    // FIXME: uncomment once rpc_impl can be instantiated with a database
    // let rpc_task = spawn(async move {
    //     if let Err(e) = rpc_server::start(&rpc_impl).await {
    //         eprintln!("Failed to start RPC server: {:?}", e);
    //     }
    // });

    // Wait for all tasks to run
    // They are supposed to run indefinitely in most cases
    // FIXME: add rpc_task here
    try_join!(duty_task)?;

    Ok(())
}

/// Pools [`BridgeDuty`]s.
///
/// This is intended to be run in an async thread with an executor as [`tokio`].
pub(crate) async fn poll_duties<L2Client>(
    operator_idx: OperatorIdx,
    bridge_duty_ops: Arc<BridgeDutyOps>,
    tx_context: Arc<TxBuildContext>,
    l2_rpc_client: Arc<L2Client>,
) -> anyhow::Result<Vec<(Txid, BridgeDuty)>>
where
    L2Client: AlpenApiClient + Send + Sync + 'static,
{
    // FIXME: be more clever about which start index to pick here
    let duties = l2_rpc_client.get_bridge_duties(operator_idx, 0).await?;

    // check which duties this operator should do something
    let mut todo_duties: Vec<(Txid, BridgeDuty)> = Vec::with_capacity(duties.duties.len());
    for duty in duties.duties {
        let txid = match &duty {
            BridgeDuty::SignDeposit(deposit) => deposit
                .construct_signing_data(&(*tx_context))
                .expect("could not build tx signing data")
                .psbt
                .compute_txid(),
            BridgeDuty::FulfillWithdrawal(withdrawal) => withdrawal
                .construct_signing_data(&(*tx_context))
                .expect("could not build tx signing data")
                .psbt
                .compute_txid(),
        };
        let status = bridge_duty_ops
            .get_status_async(txid)
            .await
            .expect("could not get contact with the bridge duty ops");
        match status {
            Some(BridgeDutyStatus::Executed) => (), // don't need to do anything
            _ => todo_duties.push((txid, duty)),    // need to do something here
        }
    }

    Ok(todo_duties)
}

/// Open or creates a rocksdb database.
///
/// # Notes
///
/// By default creates or opens a database in:
///
/// - Linux: `$HOME/.local/share/strata/rocksdb/`
/// - MacOS: `/Users/$USER/Library/Application Support/io.alpenlabs.strata/rocksdb/`
/// - Windows: `C:\Users\$USER\AppData\Roaming\alpenlabs\strata\rocksdb\data\`
///
/// Or in the specified `data_dir`
fn open_rocksdb_database(
    data_dir: Option<PathBuf>,
) -> anyhow::Result<Arc<OptimisticTransactionDB>> {
    let database_dir = match data_dir {
        Some(s) => s,
        None => ProjectDirs::from("io", "alpenlabs", "strata")
            .expect("project dir should be available")
            .data_dir()
            .to_owned()
            .join("rocksdb"),
    };

    if !database_dir.exists() {
        fs::create_dir_all(&database_dir)?;
    }

    let dbname = ROCKSDB_NAME;
    let cfs = STORE_COLUMN_FAMILIES;
    let mut opts = rocksdb::Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);

    let rbdb = OptimisticTransactionDB::open(
        &database_dir,
        dbname,
        cfs.iter().map(|s| s.to_string()),
        &opts,
    )?;

    Ok(Arc::new(rbdb))
}
