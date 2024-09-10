//! Bootstraps an RPC server for the prover client.
use std::{sync::Arc, time::Duration};

use anyhow::{Context, Ok};
use async_trait::async_trait;
use express_prover_client_rpc_api::ExpressProverClientApiServerServer;
use jsonrpsee::{core::RpcResult, RpcModule};
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::models::{ELBlockWitness, RpcContext};

pub(crate) async fn start<T>(rpc_impl: &T, rpc_url: String) -> anyhow::Result<()>
where
    T: ExpressProverClientApiServerServer + Clone,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());
    let prover_client_api = ExpressProverClientApiServerServer::into_rpc(rpc_impl.clone());
    rpc_module
        .merge(prover_client_api)
        .context("merge prover client api")?;

    info!("conneting to the server {:?}", rpc_url);
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
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<()> {
        println!("start the proving of el_block {:?}", el_block_num);

        // TODO: read the witness form the sequencer
        let witness: ELBlockWitness = Default::default();

        // Create a new proving task
        {
            let task_tracker = Arc::clone(&self.context.task_tracker);
            let task_id = task_tracker
                .create_task(el_block_num, crate::models::Witness::ElBlock(witness))
                .await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            println!("Created task: {}", task_id);
        }

        RpcResult::Ok(())
    }
}
