//! Bootstraps an RPC server for the prover client.

use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use jsonrpsee::{core::RpcResult, RpcModule};
use strata_primitives::buf::Buf32;
use strata_prover_client_rpc_api::StrataProverClientApiServer;
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::ProofKey;
use strata_state::id::L2BlockId;
use strata_state::l1::L1BlockId;
use tokio::sync::{oneshot, Mutex};
use tracing::{info, warn};

use crate::{
    operators::{ProofOperator, ProvingOp},
    task_tracker::TaskTracker,
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

/// Struct to implement the `strata_prover_client_rpc_api::StrataProverClientApiServer` on.
/// Contains fields corresponding the global context for the RPC.
#[derive(Clone)]
pub(crate) struct ProverClientRpc {
    task_tracker: Arc<Mutex<TaskTracker>>,
    operator: Arc<ProofOperator>,
    db: Arc<ProofDb>,
}

impl ProverClientRpc {
    pub fn new(
        task_tracker: Arc<Mutex<TaskTracker>>,
        operator: Arc<ProofOperator>,
        db: Arc<ProofDb>,
    ) -> Self {
        Self {
            task_tracker,
            operator,
            db,
        }
    }
}

#[async_trait]
impl StrataProverClientApiServer for ProverClientRpc {
    async fn prove_btc_block(&self, block_id: L1BlockId) -> RpcResult<Vec<ProofKey>> {
        Ok(self
            .operator
            .btc_operator()
            .create_task(block_id, self.task_tracker.clone(), &self.db)
            .await
            .expect("failed to create task"))
    }

    async fn prove_el_blocks(&self, el_block_range: (Buf32, Buf32)) -> RpcResult<Vec<ProofKey>> {
        Ok(self
            .operator
            .evm_ee_operator()
            .create_task(el_block_range, self.task_tracker.clone(), &self.db)
            .await
            .expect("failed to create task"))
    }

    async fn prove_cl_blocks(
        &self,
        cl_block_range: (L2BlockId, L2BlockId),
    ) -> RpcResult<Vec<ProofKey>> {
        Ok(self
            .operator
            .cl_stf_operator()
            .create_task(cl_block_range, self.task_tracker.clone(), &self.db)
            .await
            .expect("failed to create task"))
    }

    async fn prove_l1_batch(&self, l1_range: (L1BlockId, L1BlockId)) -> RpcResult<Vec<ProofKey>> {
        Ok(self
            .operator
            .l1_batch_operator()
            .create_task(l1_range, self.task_tracker.clone(), &self.db)
            .await
            .expect("failed to create task"))
    }

    async fn prove_l2_batch(
        &self,
        l2_range: Vec<(L2BlockId, L2BlockId)>,
    ) -> RpcResult<Vec<ProofKey>> {
        Ok(self
            .operator
            .cl_agg_operator()
            .create_task(l2_range, self.task_tracker.clone(), &self.db)
            .await
            .expect("failed to create task"))
    }

    async fn prove_latest_checkpoint(&self) -> RpcResult<Vec<ProofKey>> {
        let latest_ckp_idx = self
            .operator
            .checkpoint_operator()
            .fetch_latest_ckp_idx()
            .await
            .expect("failed to fetch latest ckp idx");
        info!(%latest_ckp_idx);
        Ok(self
            .operator
            .checkpoint_operator()
            .create_task(latest_ckp_idx, self.task_tracker.clone(), &self.db)
            .await
            .expect("failed to create task"))
    }

    async fn prove_checkpoint_raw(
        &self,
        _checkpoint_idx: u64,
        _l1_range: (u64, u64),
        _l2_range: (u64, u64),
    ) -> RpcResult<Vec<ProofKey>> {
        unimplemented!()
    }

    async fn get_task_status(&self, key: ProofKey) -> RpcResult<Option<String>> {
        let status = self.task_tracker.lock().await.get_task(key).cloned();
        match status {
            Ok(status) => RpcResult::Ok(Some(format!("{:?}", status))),
            Err(_) => RpcResult::Ok(Some(format!("{:?}", status))),
        }
    }
}
