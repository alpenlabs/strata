//! Module to bootstrap the operator node by hooking up all the required services.

use std::{path::PathBuf, sync::Arc, time::Duration};

use bitcoin::{
    key::{Keypair, Parity},
    secp256k1::{PublicKey, SecretKey, XOnlyPublicKey, SECP256K1},
};
use jsonrpsee::{core::client::async_client::Client as L2RpcClient, ws_client::WsClientBuilder};
use strata_bridge_exec::handler::ExecHandler;
use strata_bridge_sig_manager::prelude::SignatureManager;
use strata_bridge_tx_builder::prelude::TxBuildContext;
use strata_btcio::rpc::{traits::Reader, BitcoinClient};
use strata_primitives::bridge::OperatorIdx;
use strata_rocksdb::{
    bridge::db::{BridgeDutyIndexRocksDb, BridgeDutyRocksDb, BridgeTxRocksDb},
    DbOpsConfig,
};
use strata_rpc_api::StrataApiClient;
use strata_storage::ops::{
    bridge::Context as TxContext, bridge_duty::Context as DutyContext,
    bridge_duty_index::Context as DutyIndexContext,
};
use threadpool::ThreadPool;
use tracing::{error, info};

use super::{constants::DB_THREAD_COUNT, task_manager::TaskManager};
use crate::{
    args::Cli,
    constants::{
        DEFAULT_DUTY_TIMEOUT_SEC, DEFAULT_RPC_HOST, DEFAULT_RPC_PORT, ROCKSDB_RETRY_COUNT,
    },
    db::open_rocksdb_database,
    rpc_server::{self, BridgeRpc},
    xpriv::resolve_xpriv,
};

/// Bootstraps the bridge client in Operator mode by hooking up all the required auxiliary services
/// including database, rpc server, etc. Logging needs to be initialized at the call
/// site (main function) itself.
pub(crate) async fn bootstrap(args: Cli) -> anyhow::Result<()> {
    // Parse dirs
    let data_dir = args.datadir.map(PathBuf::from);

    // Initialize a rocksdb instance with the required column families.
    let rbdb = open_rocksdb_database(data_dir)?;
    let retry_count = args.retry_count.unwrap_or(ROCKSDB_RETRY_COUNT);
    let ops_config = DbOpsConfig::new(retry_count);

    // Setup Threadpool for the database I/O ops.
    let bridge_db_pool = ThreadPool::new(DB_THREAD_COUNT);

    // Setup bridge duty databases.
    let bridge_duty_db = BridgeDutyRocksDb::new(rbdb.clone(), ops_config);
    let bridge_duty_db_ctx = DutyContext::new(Arc::new(bridge_duty_db));
    let bridge_duty_db_ops = Arc::new(bridge_duty_db_ctx.into_ops(bridge_db_pool.clone()));

    let bridge_duty_idx_db = BridgeDutyIndexRocksDb::new(rbdb.clone(), ops_config);
    let bridge_duty_idx_db_ctx = DutyIndexContext::new(Arc::new(bridge_duty_idx_db));
    let bridge_duty_idx_db_ops = Arc::new(bridge_duty_idx_db_ctx.into_ops(bridge_db_pool.clone()));

    // Setup RPC clients.
    let l1_rpc_client = Arc::new(
        BitcoinClient::new(args.btc_url, args.btc_user, args.btc_pass)
            .expect("error creating the bitcoin client"),
    );
    let l2_rpc_client: L2RpcClient = WsClientBuilder::default()
        .build(args.rollup_url)
        .await
        .expect("failed to connect to the rollup RPC server");

    // Get the keypair after deriving the wallet xpriv.
    let operator_keys = resolve_xpriv(args.master_xpriv, args.master_xpriv_path)?;
    let wallet_xpriv = operator_keys.wallet_xpriv();

    let mut keypair = wallet_xpriv.to_keypair(SECP256K1);
    let mut sk = SecretKey::from_keypair(&keypair);

    // adjust for parity, which should always be even
    let (_, parity) = XOnlyPublicKey::from_keypair(&keypair);
    if matches!(parity, Parity::Odd) {
        sk = sk.negate();
        keypair = Keypair::from_secret_key(SECP256K1, &sk);
    };

    let pubkey = PublicKey::from_secret_key(SECP256K1, &sk);

    // Get this client's pubkey from the bitcoin wallet.
    let operator_pubkeys = l2_rpc_client.get_active_operator_chain_pubkey_set().await?;
    let own_index: OperatorIdx = operator_pubkeys
        .0
        .iter()
        .find_map(|(id, pk)| if pk == &pubkey { Some(*id) } else { None })
        .expect("could not find this operator's pubkey in the rollup pubkey table");

    info!(%own_index, "got own index");

    // Set up the signature manager.
    let bridge_tx_db = BridgeTxRocksDb::new(rbdb, ops_config);
    let bridge_tx_db_ctx = TxContext::new(Arc::new(bridge_tx_db));
    let bridge_tx_db_ops = Arc::new(bridge_tx_db_ctx.into_ops(bridge_db_pool));
    let sig_manager = SignatureManager::new(bridge_tx_db_ops, own_index, keypair);

    // Set up the TxBuildContext.
    let network = l1_rpc_client.network().await?;
    let tx_context = TxBuildContext::new(network, operator_pubkeys, own_index);

    // Spawn RPC server.
    let bridge_rpc = BridgeRpc::new(bridge_duty_db_ops.clone());

    let rpc_host = args.rpc_host.as_deref().unwrap_or(DEFAULT_RPC_HOST);
    let rpc_port = args.rpc_port.unwrap_or(DEFAULT_RPC_PORT);
    let rpc_addr = format!("{rpc_host}:{rpc_port}");

    let rpc_task = tokio::spawn(async move {
        if let Err(e) = rpc_server::start(&bridge_rpc, rpc_addr.as_str()).await {
            error!(error = %e, "could not start RPC server");
        }
    });

    let rollup_block_time = l2_rpc_client
        .block_time()
        .await
        .expect("should be able to get block time from rollup RPC client");

    let msg_polling_interval = args.message_interval.map_or(
        Duration::from_millis(rollup_block_time / 2),
        Duration::from_millis,
    );

    // Spawn poll duties task.
    let exec_handler = ExecHandler {
        tx_build_ctx: tx_context,
        sig_manager,
        l2_rpc_client,
        keypair,
        own_index,
        msg_polling_interval,
    };

    let task_manager = TaskManager {
        exec_handler: Arc::new(exec_handler),
        broadcaster: l1_rpc_client,
        bridge_duty_db_ops,
        bridge_duty_idx_db_ops,
    };

    let duty_polling_interval = args.duty_interval.map_or(
        Duration::from_millis(rollup_block_time),
        Duration::from_millis,
    );

    let duty_timeout_duration = Duration::from_secs(
        args.duty_timeout_duration
            .unwrap_or(DEFAULT_DUTY_TIMEOUT_SEC),
    );

    // TODO: wrap these in `strata-tasks`
    let duty_task = tokio::spawn(async move {
        if let Err(e) = task_manager
            .start(duty_polling_interval, duty_timeout_duration)
            .await
        {
            error!(error = %e, "could not start task manager");
        };
    });

    // Wait for all tasks to run
    // They are supposed to run indefinitely in most cases
    tokio::try_join!(rpc_task, duty_task)?;

    Ok(())
}
