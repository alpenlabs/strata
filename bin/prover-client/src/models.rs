use std::sync::Arc;

use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zkvm_primitives::ZKVMInput;

use crate::task_tracker::TaskTracker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Witness {
    ElWitness(ZKVMInput),
    Mock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Uuid,
    pub el_block_num: u64,
    pub witness: Witness,
    pub status: TaskStatus,
}

#[derive(Clone)]
pub struct RpcContext {
    pub task_tracker: Arc<TaskTracker>,
    sequencer_rpc_url: String,
    reth_rpc_url: String,
    el_rpc_client: HttpClient,
}

impl RpcContext {
    pub fn new(
        task_tracker: Arc<TaskTracker>,
        sequencer_rpc_url: String,
        reth_rpc_url: String,
    ) -> Self {
        let el_rpc_client = HttpClientBuilder::default()
            .build(&reth_rpc_url)
            .expect("failed to connect to the el client");

        RpcContext {
            task_tracker,
            sequencer_rpc_url,
            reth_rpc_url,
            el_rpc_client,
        }
    }

    pub fn el_client(&self) -> &HttpClient {
        &self.el_rpc_client
    }
}
