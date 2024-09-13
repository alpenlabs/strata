//! Bootstraps an RPC server for the prover client.
use std::sync::Arc;

use anyhow::{Context, Ok};
use async_trait::async_trait;
use express_prover_client_rpc_api::ExpressProverClientApiServerServer;
use jsonrpsee::{
    core::{client::ClientT, RpcResult},
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params, RpcModule,
};
use reth_rpc_types::Block;
use tokio::sync::oneshot;
use tracing::{info, warn};
use zkvm_primitives::ZKVMInput;

use crate::{
    primitives::prover_input::{ProverInput, WitnessData},
    task_tracker::TaskTracker,
};

#[derive(Clone)]
pub struct RpcContext {
    pub task_tracker: Arc<TaskTracker>,
    _sequencer_rpc_url: String,
    el_rpc_client: HttpClient,
}

impl RpcContext {
    pub fn new(
        task_tracker: Arc<TaskTracker>,
        _sequencer_rpc_url: String,
        reth_rpc_url: String,
    ) -> Self {
        let el_rpc_client = HttpClientBuilder::default()
            .build(&reth_rpc_url)
            .expect("failed to connect to the el client");

        RpcContext {
            task_tracker,
            _sequencer_rpc_url,
            el_rpc_client,
        }
    }

    pub fn el_client(&self) -> &HttpClient {
        &self.el_rpc_client
    }
}

pub(crate) async fn start<T>(rpc_impl: &T, rpc_url: String) -> anyhow::Result<()>
where
    T: ExpressProverClientApiServerServer + Clone,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());
    let prover_client_api = ExpressProverClientApiServerServer::into_rpc(rpc_impl.clone());
    rpc_module
        .merge(prover_client_api)
        .context("merge prover client api")?;

    info!("connecting to the server {:?}", rpc_url);
    let rpc_server = jsonrpsee::server::ServerBuilder::new()
        .build(&rpc_url)
        .await
        .expect("build prover rpc server");

    let rpc_handle = rpc_server.start(rpc_module);
    let (_stop_tx, stop_rx): (oneshot::Sender<bool>, oneshot::Receiver<bool>) = oneshot::channel();
    info!("prover client  RPC server started at: {}", rpc_url);

    let _ = stop_rx.await;
    info!("stopping RPC server");

    if rpc_handle.stop().is_err() {
        warn!("rpc server already stopped");
    }

    Ok(())
}

/// Struct to implement the `express_prover_client_rpc_api::ExpressProverClientApiServer` on.
/// Contains fields corresponding the global context for the RPC.
#[derive(Clone)]
pub(crate) struct ProverClientRpc {
    context: RpcContext,
}

impl ProverClientRpc {
    pub fn new(context: RpcContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl ExpressProverClientApiServerServer for ProverClientRpc {
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<String> {
        // TODO: handle the unwrap here
        let zkvm_input = self
            .fetch_el_block_witness(el_block_num)
            .await
            .expect("Failed to get th el block witness from the reth rpc");

        let el_block_witness = WitnessData {
            data: bincode::serialize(&zkvm_input).unwrap(),
        };
        let witness = ProverInput::ElBlock(el_block_witness);

        let task_tracker = Arc::clone(&self.context.task_tracker);
        let task_id = task_tracker.create_task(el_block_num, witness).await;

        RpcResult::Ok(task_id.to_string())
    }
}

impl ProverClientRpc {
    async fn fetch_el_block_witness(&self, el_block_num: u64) -> anyhow::Result<ZKVMInput> {
        let el_rpc_client = self.context.el_client();

        let el_block: Block = el_rpc_client
            .request(
                "eth_getBlockByNumber",
                rpc_params![format!("0x{:x}", el_block_num), false],
            )
            .await
            .context("Failed to get the el block")?;

        let el_block_witness: ZKVMInput = el_rpc_client
            .request(
                "alpee_getBlockWitness",
                rpc_params![el_block.header.hash.context("Block hash missing")?, true],
            )
            .await
            .context("Failed to get the EL witness")?;

        Ok(el_block_witness)
    }
}
