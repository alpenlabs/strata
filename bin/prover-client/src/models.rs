use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::task_tracker::TaskTracker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ELBlockWitness {
    pub data: Vec<u8>,
}

impl Default for ELBlockWitness {
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
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
    pub witness: ELBlockWitness,
    pub status: TaskStatus,
}

#[derive(Clone)]
pub struct RpcContext {
    pub task_tracker: Arc<TaskTracker>,
}

impl RpcContext {
    pub fn new(task_tracker: Arc<TaskTracker>) -> Self {
        RpcContext { task_tracker }
    }
}
