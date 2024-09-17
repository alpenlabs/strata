//! Bootstraps an RPC server for the prover client.

use anyhow::{Context, Ok};
use async_trait::async_trait;
use express_prover_client_rpc_api::ExpressProverClientApiServerServer;
use jsonrpsee::{core::RpcResult, RpcModule};
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::task_dispatcher::ELBlockProvingTaskScheduler;

#[derive(Clone)]
pub struct RpcContext {
    pub el_proving_task_scheduler: ELBlockProvingTaskScheduler,
}

impl RpcContext {
    pub fn new(el_proving_task_scheduler: ELBlockProvingTaskScheduler) -> Self {
        Self {
            el_proving_task_scheduler,
        }
    }
}

pub(crate) async fn start<T>(
    rpc_impl: &T,
    rpc_url: String,
    enable_dev_rpc: bool,
) -> anyhow::Result<()>
where
    T: ExpressProverClientApiServerServer + Clone,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());

    if enable_dev_rpc {
        let prover_client_dev_api = ExpressProverClientApiServerServer::into_rpc(rpc_impl.clone());
        rpc_module
            .merge(prover_client_dev_api)
            .context("merge prover client api")?;
    }

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
        let task_id = self
            .context
            .el_proving_task_scheduler
            .create_proving_task(el_block_num)
            .await
            .expect("failed to add proving task");

        RpcResult::Ok(task_id.to_string())
    }
}
