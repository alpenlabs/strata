//! Bootstraps an RPC server for the prover client.

use std::sync::Arc;

use anyhow::{Context, Ok};
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, RpcModule};
use strata_primitives::proof::ProofKey;
use strata_prover_client_rpc_api::StrataProverClientApiServer;
use strata_rpc_types::RpcCheckpointInfo;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    errors::ProvingTaskError, manager2::ProverManager, primitives::status::ProvingTaskStatus,
    task2::TaskTracker,
};

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

pub enum ServerRequest {
    AddTask {
        task: ProofKey,
        response_tx: oneshot::Sender<ProvingTaskStatus>,
    },
    GetTaskStatus {
        task: ProofKey,
        response_tx: oneshot::Sender<ProvingTaskStatus>,
    },
}

/// Struct to implement the `strata_prover_client_rpc_api::StrataProverClientApiServer` on.
/// Contains fields corresponding the global context for the RPC.
#[derive(Clone)]
pub(crate) struct ProverClientRpc {
    task_tx: mpsc::Sender<ServerRequest>,
}

impl ProverClientRpc {
    pub fn new(task_tx: mpsc::Sender<ServerRequest>) -> Self {
        Self { task_tx }
    }

    pub async fn add_task(
        &self,
        task: ProofKey,
    ) -> Result<ProvingTaskStatus, mpsc::error::SendError<ServerRequest>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.task_tx
            .send(ServerRequest::AddTask { task, response_tx })
            .await?;

        // Wait for the initial task status
        response_rx.await.map_err(|_| {
            mpsc::error::SendError(ServerRequest::AddTask {
                task,
                response_tx: oneshot::channel().0,
            })
        })
    }

    pub async fn get_task_status(
        &self,
        task: ProofKey,
    ) -> Result<ProvingTaskStatus, mpsc::error::SendError<ServerRequest>> {
        let (response_tx, response_rx) = oneshot::channel();

        self.task_tx
            .send(ServerRequest::GetTaskStatus { task, response_tx })
            .await?;

        // Wait for the task status
        response_rx.await.map_err(|_| {
            mpsc::error::SendError(ServerRequest::GetTaskStatus {
                task,
                response_tx: oneshot::channel().0,
            })
        })
    }
}

#[async_trait]
impl StrataProverClientApiServer for ProverClientRpc {
    async fn prove_btc_block(&self, btc_block_num: u64) -> RpcResult<Uuid> {
        let task = ProofKey::BtcBlockspace(btc_block_num);
        let _ = self
            .add_task(task)
            .await
            .expect("failed to add proving task, btc block");
        RpcResult::Ok(Uuid::new_v4())
    }

    async fn prove_el_block(&self, el_block_num: u64) -> RpcResult<Uuid> {
        let task = ProofKey::EvmEeStf(el_block_num);
        let _ = self
            .add_task(task)
            .await
            .expect("failed to add proving task, el block");
        RpcResult::Ok(Uuid::new_v4())
    }

    async fn prove_cl_block(&self, cl_block_num: u64) -> RpcResult<Uuid> {
        let task = ProofKey::ClStf(cl_block_num);
        let _ = self
            .add_task(task)
            .await
            .expect("failed to add proving task, el block");
        RpcResult::Ok(Uuid::new_v4())
    }

    async fn prove_l1_batch(&self, l1_range: (u64, u64)) -> RpcResult<Uuid> {
        let task = ProofKey::L1Batch(l1_range.0, l1_range.1);
        let _ = self
            .add_task(task)
            .await
            .expect("failed to add proving task, el block");
        RpcResult::Ok(Uuid::new_v4())
    }

    async fn prove_l2_batch(&self, l2_range: (u64, u64)) -> RpcResult<Uuid> {
        let task = ProofKey::ClAgg(l2_range.0, l2_range.1);
        let _ = self
            .add_task(task)
            .await
            .expect("failed to add proving task, el block");
        RpcResult::Ok(Uuid::new_v4())
    }

    async fn prove_latest_checkpoint(&self) -> RpcResult<Uuid> {
        unimplemented!()
    }

    async fn prove_checkpoint_raw(
        &self,
        checkpoint_idx: u64,
        l1_range: (u64, u64),
        l2_range: (u64, u64),
    ) -> RpcResult<Uuid> {
        unimplemented!()
    }

    async fn get_task_status(&self, task_id: Uuid) -> RpcResult<Option<String>> {
        unimplemented!()
    }
}
