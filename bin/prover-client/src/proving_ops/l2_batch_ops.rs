use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use uuid::Uuid;

use super::{cl_ops::ClOperations, ops::ProvingOperations};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{ProofWithVkey, ZKVMInput},
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct L2BatchOperations {
    cl_dispatcher: Arc<TaskDispatcher<ClOperations>>,
}

impl L2BatchOperations {
    /// Creates a new BTC operations instance.
    pub fn new(cl_dispatcher: Arc<TaskDispatcher<ClOperations>>) -> Self {
        Self { cl_dispatcher }
    }
}

#[derive(Debug, Clone)]
pub struct L2BatchInput {
    pub cl_block_range: (u64, u64),
    pub cl_task_ids: HashMap<Uuid, u64>,
    pub proofs: HashMap<u64, ProofWithVkey>,
}

impl L2BatchInput {
    pub fn insert_proof(&mut self, cl_task_id: Uuid, proof: ProofWithVkey) {
        if let Some(cl_idx) = self.cl_task_ids.get(&cl_task_id) {
            self.proofs.insert(*cl_idx, proof);
        }
    }

    pub fn get_proofs(&self) -> Vec<ProofWithVkey> {
        let mut proofs = Vec::new();

        let (start, end) = self.cl_block_range;
        for cl_block_idx in start..=end {
            let proof = self.proofs.get(&cl_block_idx).unwrap();
            proofs.push(proof.clone());
        }

        proofs
    }
}

#[async_trait]
impl ProvingOperations for L2BatchOperations {
    // Range of l1 blocks
    type Input = L2BatchInput;
    type Params = (u64, u64);

    fn block_type(&self) -> ProvingTaskType {
        ProvingTaskType::ClBatch
    }

    async fn fetch_input(
        &self,
        cl_block_range: Self::Params,
    ) -> Result<Self::Input, anyhow::Error> {
        // Init the proof
        let proofs: HashMap<u64, ProofWithVkey> = HashMap::new();

        let cl_task_ids = HashMap::new();
        let input: Self::Input = L2BatchInput {
            cl_block_range,
            cl_task_ids,
            proofs,
        };
        Ok(input)
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        mut input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let mut dependencies = vec![];

        // Create CL tasks for each block in the l2 range
        let (start, end) = input.cl_block_range;
        for cl_block_idx in start..=end {
            let cl_task_id = self
                .cl_dispatcher
                .create_task(cl_block_idx)
                .await
                .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
            dependencies.push(cl_task_id);
            input.cl_task_ids.insert(cl_task_id, cl_block_idx);
        }

        // Create the l2_batch task with dependencies on CL tasks
        let task_id = task_tracker
            .create_task(ZKVMInput::L2Batch(input), dependencies)
            .await;
        Ok(task_id)
    }
}
