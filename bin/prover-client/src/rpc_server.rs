//! Bootstraps an RPC server for the prover client.

use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, RpcModule};
use strata_primitives::proof::ProofKey;
use strata_prover_client_rpc_api::StrataProverClientApiServer;
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use crate::{handlers::ProofHandler, task2::TaskTracker};

pub(crate) async fn start<T>(
    rpc_impl: &T,
    rpc_url: String,
    enable_dev_rpc: bool,
) -> anyhow::Result<()>
where
    T: StrataProverClientApiServer + Clone,
{
    let mut rpc_module = RpcModule::new(rpc_impl.clone());

    if enable_dev_rpc {
        let prover_client_dev_api = StrataProverClientApiServer::into_rpc(rpc_impl.clone());
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

/// Struct to implement the `strata_prover_client_rpc_api::StrataProverClientApiServer` on.
/// Contains fields corresponding the global context for the RPC.
#[derive(Clone)]
pub(crate) struct ProverClientRpc {
    task_tracker: Arc<Mutex<TaskTracker>>,
    handler: Arc<ProofHandler>,
}

impl ProverClientRpc {
    pub fn new(task_tracker: Arc<Mutex<TaskTracker>>, handler: Arc<ProofHandler>) -> Self {
        Self {
            task_tracker,
            handler,
        }
    }

    pub async fn create_task(&self, task: ProofKey) -> RpcResult<ProofKey> {
        self.handler
            .create_task(self.task_tracker.clone(), &task)
            .await
            .expect("failed to add proving task");
        RpcResult::Ok(task)
    }
}

#[async_trait]
impl StrataProverClientApiServer for ProverClientRpc {
    async fn prove_btc_block(&self, btc_block_num: u64) -> RpcResult<ProofKey> {
        let task = ProofKey::BtcBlockspace(btc_block_num);
        self.create_task(task).await
    }

    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<ProofKey> {
        let task = ProofKey::BtcBlockspace(el_block_num);
        self.create_task(task).await
    }

    async fn prove_cl_block(&self, cl_block_num: u64) -> RpcResult<ProofKey> {
        let task = ProofKey::ClStf(cl_block_num);
        self.create_task(task).await
    }

    async fn prove_l1_batch(&self, l1_range: (u64, u64)) -> RpcResult<ProofKey> {
        let task = ProofKey::L1Batch(l1_range.0, l1_range.1);
        self.create_task(task).await
    }

    async fn prove_l2_batch(&self, l2_range: (u64, u64)) -> RpcResult<ProofKey> {
        let task = ProofKey::ClAgg(l2_range.0, l2_range.1);
        self.create_task(task).await
    }

    async fn prove_latest_checkpoint(&self) -> RpcResult<ProofKey> {
        unimplemented!()
    }

    async fn prove_checkpoint_raw(
        &self,
        _checkpoint_idx: u64,
        _l1_range: (u64, u64),
        _l2_range: (u64, u64),
    ) -> RpcResult<ProofKey> {
        unimplemented!()
    }

    async fn get_task_status(&self, id: ProofKey) -> RpcResult<Option<String>> {
        let status = self.task_tracker.lock().await.get_task(id).cloned();
        match status {
            Ok(status) => RpcResult::Ok(Some(format!("{:?}", status))),
            Err(_) => RpcResult::Ok(Some(format!("{:?}", status))),
        }
    }
}
