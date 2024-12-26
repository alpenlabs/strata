use std::sync::Arc;

use strata_btcio::component::{L1Reader, L1ReaderBuilder};
use strata_component::{
    context::BuildContext,
    // reader::L1Reader,
};
use strata_consensus_logic::checkpoint::CheckpointHandle;
use strata_rocksdb::{init_core_dbs, open_rocksdb_database, DbOpsConfig, ROCKSDB_NAME};
use strata_storage::managers::create_db_manager;
use strata_tasks::TaskManager;
use tracing::info;

use crate::{
    args::Args, builder::ClientBuilder, client::Client, create_bitcoin_rpc_client, get_config,
    init_status_channel, resolve_and_validate_params,
};

pub fn main_inner(args: Args) -> anyhow::Result<()> {
    // Load and validate configuration and params
    let config = get_config(args.clone())?;
    // Set up block params.
    let params = resolve_and_validate_params(args.rollup_params.as_deref(), &config)
        .map_err(anyhow::Error::from)?;

    // Init the task manager and logging before we do anything else.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("strata-rt")
        .build()
        .expect("init: build rt");
    let task_manager = TaskManager::new(runtime.handle().clone());
    let executor = task_manager.executor();

    // init_logging(executor.handle());

    // Init thread pool for batch jobs.
    // TODO switch to num_cpus
    let pool = threadpool::ThreadPool::with_name("strata-pool".to_owned(), 8);

    // Open and initialize rocksdb.
    let rbdb = open_rocksdb_database(&config.client.datadir, ROCKSDB_NAME)?;
    let ops_config = DbOpsConfig::new(config.client.db_retry_count);

    // Initialize core databases and validate
    let database = init_core_dbs(rbdb.clone(), ops_config);
    let manager = create_db_manager(database.clone(), pool.clone());
    let build_ctx = BuildContext::new(config.clone(), (*params).clone(), manager.clone());
    let status_channel = init_status_channel(database.as_ref())?;

    let client_builder = ClientBuilder::default().with_reader(L1ReaderBuilder);

    let (client, csm_context): (Client<L1Reader, (), (), ()>, _) = client_builder
        .build_and_validate(
            build_ctx,
            task_manager,
            status_channel,
            database.clone(),
            pool.clone(),
        );

    client.do_genesis(&csm_context, database)?;
    let cl_handle = strata_component::Client::run(&client, &csm_context);

    // create rpcs
    // and wait forever, like a mad lover

    // BRIDGE SIDECAR possibly
    // Set up bridge messaging stuff.
    // TODO move all of this into relayer task init
    let bridge_msg_db = Arc::new(strata_rocksdb::BridgeMsgDb::new(rbdb.clone(), ops_config));
    let bridge_msg_ctx = strata_storage::ops::bridge_relay::Context::new(bridge_msg_db);
    let bridge_msg_ops = Arc::new(bridge_msg_ctx.into_ops(pool.clone()));
    // BRIDGE SIDECAR END

    // Checkpoint handle can be separated out as this is used by sequencer specific tasks and
    // rpcs
    let checkpoint_handle: Arc<_> = CheckpointHandle::new(manager.checkpoint()).into();
    let bitcoin_client = create_bitcoin_rpc_client(&config)?;

    info!("init finished, starting main tasks");

    // let ctx = start_core_tasks(
    //     &executor,
    //     pool,
    //     &config,
    //     params.clone(),
    //     database,
    //     &manager,
    //     bridge_msg_ops,
    //     bitcoin_client,
    // )?;

    // let mut methods = jsonrpsee::Methods::new();

    // match &config.client.client_mode {
    //     // If we're a sequencer, start the sequencer db and duties task.
    //     ClientMode::Sequencer(sequencer_config) => {
    //         let broadcast_database = init_broadcaster_database(rbdb.clone(), ops_config);
    //         let broadcast_handle = start_broadcaster_tasks(
    //             broadcast_database,
    //             ctx.pool.clone(),
    //             &executor,
    //             ctx.bitcoin_client.clone(),
    //             params.clone(),
    //         );
    //         let seq_db = init_sequencer_database(rbdb.clone(), ops_config);

    //         start_sequencer_tasks(
    //             ctx.clone(),
    //             &config,
    //             sequencer_config,
    //             &executor,
    //             seq_db,
    //             checkpoint_handle.clone(),
    //             broadcast_handle,
    //             &mut methods,
    //         )?;
    //     }
    //     ClientMode::FullNode(fullnode_config) => {
    //         let sequencer_rpc = &fullnode_config.sequencer_rpc;
    //         info!(?sequencer_rpc, "initing fullnode task");

    //         let rpc_client = executor.handle().block_on(sync_client(sequencer_rpc));
    //         let sync_peer = RpcSyncPeer::new(rpc_client, 10);
    //         let l2_sync_context = L2SyncContext::new(
    //             sync_peer,
    //             ctx.l2_block_manager.clone(),
    //             ctx.sync_manager.clone(),
    //         );
    //         // NOTE: this might block for some time during first run with empty db until
    // genesis         // block is generated
    //         let mut l2_sync_state =
    //             strata_sync::block_until_csm_ready_and_init_sync_state(&l2_sync_context)?;

    //         executor.spawn_critical_async("l2-sync-manager", async move {
    //             strata_sync::sync_worker(&mut l2_sync_state, &l2_sync_context)
    //                 .await
    //                 .map_err(Into::into)
    //         });
    //     }
    // }

    // executor.spawn_critical_async(
    //     "main-rpc",
    //     start_rpc(
    //         ctx,
    //         task_manager.shutdown_signal(),
    //         config,
    //         checkpoint_handle,
    //         methods,
    //     ),
    // );

    // task_manager.start_signal_listeners();
    // task_manager.monitor(Some(Duration::from_secs(5)))?;

    info!("exiting");
    Ok(())
}
