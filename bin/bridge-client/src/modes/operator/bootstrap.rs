//! Module to bootstrap the operator node by hooking up all the required services.

use std::{env, path::PathBuf, sync::Arc, time::Duration};

use bitcoin::{
    key::{Keypair, Parity},
    secp256k1::{PublicKey, SecretKey, XOnlyPublicKey, SECP256K1},
};
use deadpool::managed;
use strata_bridge_exec::{
    handler::ExecHandler,
    ws_client::{WsClientConfig, WsClientManager},
};
use strata_bridge_sig_manager::prelude::SignatureManager;
use strata_bridge_tx_builder::prelude::TxBuildContext;
use strata_btcio::rpc::{traits::ReaderRpc, BitcoinClient};
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
        DEFAULT_DUTY_TIMEOUT_SEC, DEFAULT_MAX_RPC_RETRY_COUNT, DEFAULT_ROCKSDB_RETRY_COUNT,
        DEFAULT_RPC_HOST, DEFAULT_RPC_PORT,
    },
    db::open_rocksdb_database,
    rpc_server::{self, BridgeRpc},
    xpriv::{resolve_xpriv, OPXPRIV_ENVVAR},
};

// TODO: move this to some common util and make this usable outside tokio
macro_rules! retry {
    ($expr:expr) => {
        retry!(5, $expr)
    };
    ($max:expr, $expr:expr) => {{
        let mut attempts = 0;
        loop {
            match $expr {
                Ok(val) => break Ok(val),
                Err(err) => {
                    attempts += 1;
                    if attempts >= $max {
                        break Err(err);
                    }
                    ::tokio::time::sleep(::core::time::Duration::from_secs(2)).await
                }
            }
        }
    }};
}

/// Bootstraps the bridge client in Operator mode by hooking up all the required auxiliary services
/// including database, rpc server, etc. Logging needs to be initialized at the call
/// site (main function) itself.
pub(crate) async fn bootstrap(args: Cli) -> anyhow::Result<()> {
    // Parse dirs
    let data_dir = args.datadir.map(PathBuf::from);

    // Initialize a rocksdb instance with the required column families.
    let rbdb = open_rocksdb_database(data_dir)?;
    let retry_count = args.retry_count.unwrap_or(DEFAULT_ROCKSDB_RETRY_COUNT);
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
        BitcoinClient::new(
            args.btc_url,
            args.btc_user,
            args.btc_pass,
            args.btc_retry_count,
            args.btc_retry_interval,
        )
        .expect("error creating the bitcoin client"),
    );

    let config = WsClientConfig {
        url: args.rollup_url.clone(),
    };
    let manager = WsClientManager { config };
    let l2_rpc_client_pool = managed::Pool::<WsClientManager>::builder(manager)
        .max_size(5)
        .build()
        .unwrap();

    let l2_rpc_client = l2_rpc_client_pool
        .get()
        .await
        .expect("cannot get RPC client from pool");

    // Get the keypair after deriving the wallet xpriv.
    let env_key = match env::var(OPXPRIV_ENVVAR) {
        Ok(k) => Some(k),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            error!("operator master xpriv envvar not unicode, ignoring");
            None
        }
    };

    let operator_keys = resolve_xpriv(args.master_xpriv, args.master_xpriv_path, env_key)?;
    let wallet_xpriv = operator_keys.wallet_xpriv();

    let mut keypair = wallet_xpriv.to_keypair(SECP256K1);
    let mut sk = SecretKey::from_keypair(&keypair);

    // adjust for parity, which should always be even
    // FIXME bake this into the key derivation fn!
    let (_, parity) = XOnlyPublicKey::from_keypair(&keypair);
    if matches!(parity, Parity::Odd) {
        sk = sk.negate();
        keypair = Keypair::from_secret_key(SECP256K1, &sk);
    };

    let pubkey = PublicKey::from_secret_key(SECP256K1, &sk);

    // Get this client's pubkey from the bitcoin wallet.
    let operator_pubkeys = retry!(l2_rpc_client.get_active_operator_chain_pubkey_set().await)?;
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
    let sig_manager = SignatureManager::new(bridge_tx_db_ops, own_index, keypair.into());

    // Set up the TxBuildContext.
    let network = retry!(l1_rpc_client.network().await)?;
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
        l2_rpc_client_pool,
        keypair: keypair.into(),
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

    let max_retry_count = args
        .max_rpc_retry_count
        .unwrap_or(DEFAULT_MAX_RPC_RETRY_COUNT);

    // TODO: wrap these in `strata-tasks`
    let duty_task = tokio::spawn(async move {
        if let Err(e) = task_manager
            .start(
                duty_polling_interval,
                duty_timeout_duration,
                max_retry_count,
            )
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
