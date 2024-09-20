mod db;

use std::{future::Future, sync::Arc};

use clap::Parser;
use eyre::Ok;
use reth::{
    args::LogArgs,
    builder::{NodeBuilder, WithLaunchContext},
    CliRunner,
};
use reth_chainspec::ChainSpec;
use reth_cli_commands::node::NodeCommand;
use reth_primitives::Genesis;
use strata_reth_db::rocksdb::WitnessDB;
use strata_reth_exex::ProverWitnessGenerator;
use strata_reth_node::StrataEthereumNode;
use strata_reth_rpc::{AlpenRPC, AlpenRpcApiServer};
use tracing::info;

const ALPEN_CHAIN_SPEC: &str = include_str!("../res/alpen-dev-chain.json");

fn main() {
    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    let mut command = NodeCommand::<AdditionalConfig>::parse();
    // use provided alpen chain spec
    command.chain = parse_chain_spec(ALPEN_CHAIN_SPEC).expect("valid chainspec");
    // disable peer discovery
    command.network.discovery.disable_discovery = true;

    if let Err(err) = run(command, |builder, ext| async move {
        let datadir = builder.config().datadir().data_dir().to_path_buf();
        let mut node_builder = builder.node(StrataEthereumNode::default());

        // Install Prover Input ExEx, persist to DB, and add RPC for querying block witness.
        if ext.enable_witness_gen {
            let rbdb = db::open_rocksdb_database(datadir.clone()).expect("open rocksdb");
            let db = Arc::new(WitnessDB::new(rbdb));
            let rpc_db = db.clone();

            node_builder = node_builder
                .extend_rpc_modules(|ctx| {
                    let alpen_rpc = AlpenRPC::new(rpc_db);
                    ctx.modules.merge_configured(alpen_rpc.into_rpc())?;

                    Ok(())
                })
                .install_exex("prover_input", |ctx| async {
                    Ok(ProverWitnessGenerator::new(ctx, db).start())
                });
        }

        let handle = node_builder.launch().await?;
        handle.node_exit_future.await
    }) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

/// Our custom cli args extension that adds one flag to reth default CLI.
#[derive(Debug, clap::Parser)]
pub struct AdditionalConfig {
    #[command(flatten)]
    pub logs: LogArgs,

    #[arg(long, default_value_t = false)]
    pub enable_witness_gen: bool,
}

fn parse_chain_spec(chain_json: &str) -> eyre::Result<Arc<ChainSpec>> {
    // both serialized Genesis and ChainSpec structs supported
    let genesis: Genesis = serde_json::from_str(chain_json)?;

    Ok(Arc::new(genesis.into()))
}

/// Run node with logging
/// based on reth::cli::Cli::run
fn run<L, Fut>(mut command: NodeCommand<AdditionalConfig>, launcher: L) -> eyre::Result<()>
where
    L: FnOnce(WithLaunchContext<NodeBuilder<Arc<reth_db::DatabaseEnv>>>, AdditionalConfig) -> Fut,
    Fut: Future<Output = eyre::Result<()>>,
{
    command.ext.logs.log_file_directory = command
        .ext
        .logs
        .log_file_directory
        .join(command.chain.chain.to_string());

    let _guard = command.ext.logs.init_tracing()?;
    info!(target: "reth::cli", cmd = %command.ext.logs.log_file_directory, "Initialized tracing, debug log directory");

    let runner = CliRunner::default();
    runner.run_command_until_exit(|ctx| command.execute(ctx, launcher))?;

    Ok(())
}
