//! Bootstraps an RPC server for the prover client.
use anyhow::{Context, Ok};
use async_trait::async_trait;
use express_prover_client_rpc_api::ExpressProverClientApiServerServer;
use jsonrpsee::{core::RpcResult, RpcModule};
use tokio::sync::oneshot;
use tracing::{info, warn};

use crate::constants::{RPC_PORT, RPC_SERVER};

pub(crate) async fn start<T>(rpc_impl: &T) -> anyhow::Result<()>
where
    T: ExpressProverClientApiServerServer + Clone,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());
    let prover_client_api = ExpressProverClientApiServerServer::into_rpc(rpc_impl.clone());
    rpc_module
        .merge(prover_client_api)
        .context("merge prover client api")?;

    let addr = format!("{RPC_SERVER}:{RPC_PORT}");
    let rpc_server = jsonrpsee::server::ServerBuilder::new()
        .build(&addr)
        .await
        .expect("build bridge rpc server");

    let rpc_handle = rpc_server.start(rpc_module);
    let (_stop_tx, stop_rx): (oneshot::Sender<bool>, oneshot::Receiver<bool>) = oneshot::channel();
    info!("prover client  RPC server started at: {addr}");

    let _ = stop_rx.await;
    info!("stopping RPC server");

    if rpc_handle.stop().is_err() {
        warn!("rpc server already stopped");
    }

    Ok(())
}

/// Struct to implement the `express_prover_client_rpc_api::ExpressProverClientApiServer` on.
/// Contains fields corresponding the global context for the RPC.
#[derive(Debug, Clone)]
pub(crate) struct ProverClientRpc {}

impl ProverClientRpc {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl ExpressProverClientApiServerServer for ProverClientRpc {
    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<()> {
        println!("start the proving of el_block {:?}", el_block_num);
        RpcResult::Ok(())
    }
}
