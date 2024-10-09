use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use bitcoin::params::MAINNET;
use strata_btcio::{reader::query::get_verification_state, rpc::BitcoinClient};
use strata_primitives::params::RollupParams;
use strata_state::l1::HeaderVerificationState;
use uuid::Uuid;

use super::{
    btc_ops::{get_pm_rollup_params, BtcOperations},
    ops::ProvingOperations,
};
use crate::{
    dispatcher::TaskDispatcher,
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{ProofWithVkey, ZKVMInput},
    task::TaskTracker,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct L1BatchOperations {
    btc_dispatcher: Arc<TaskDispatcher<BtcOperations>>,
    btc_client: Arc<BitcoinClient>,
}

impl L1BatchOperations {
    /// Creates a new BTC operations instance.
    pub fn new(
        btc_dispatcher: Arc<TaskDispatcher<BtcOperations>>,
        btc_client: Arc<BitcoinClient>,
    ) -> Self {
        Self {
            btc_dispatcher,
            btc_client,
        }
    }
}

#[derive(Debug, Clone)]
pub struct L1BatchInput {
    pub btc_block_range: (u64, u64),
    pub btc_task_ids: HashMap<Uuid, u64>,
    pub proofs: HashMap<u64, ProofWithVkey>,
    pub header_verification_state: HeaderVerificationState,
    pub rollup_params: RollupParams,
}

impl L1BatchInput {
    pub fn insert_proof(&mut self, btc_task_id: Uuid, proof: ProofWithVkey) {
        if let Some(btc_blk_idx) = self.btc_task_ids.get(&btc_task_id) {
            self.proofs.insert(*btc_blk_idx, proof);
        }
    }

    pub fn get_proofs(&self) -> Vec<ProofWithVkey> {
        let mut proofs = Vec::new();

        let (start, end) = self.btc_block_range;
        for btc_block_idx in start..=end {
            let proof = self.proofs.get(&btc_block_idx).unwrap();
            proofs.push(proof.clone());
        }

        proofs
    }
}

#[async_trait]
impl ProvingOperations for L1BatchOperations {
    // Range of l1 blocks
    type Input = L1BatchInput;
    type Params = (u64, u64);

    fn proving_task_type(&self) -> ProvingTaskType {
        ProvingTaskType::BtcBatch
    }

    async fn fetch_input(
        &self,
        btc_block_range: Self::Params,
    ) -> Result<Self::Input, anyhow::Error> {
        let st_height = btc_block_range.0;
        let header_verification_state =
            get_verification_state(self.btc_client.as_ref(), st_height, &MAINNET.clone().into())
                .await?;
        let rollup_params = get_pm_rollup_params();

        let input: Self::Input = L1BatchInput {
            btc_block_range,
            btc_task_ids: HashMap::new(),
            proofs: HashMap::new(),
            header_verification_state,
            rollup_params,
        };
        Ok(input)
    }

    async fn append_task(
        &self,
        task_tracker: Arc<TaskTracker>,
        mut input: Self::Input,
    ) -> Result<Uuid, ProvingTaskError> {
        let mut dependencies = vec![];

        // Create btc tasks for each block in the range
        let (start, end) = input.btc_block_range;
        for btc_block_idx in start..=end {
            let btc_task_id = self
                .btc_dispatcher
                .create_task(btc_block_idx)
                .await
                .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
            dependencies.push(btc_task_id);
            input.btc_task_ids.insert(btc_task_id, btc_block_idx);
        }

        // Create the l1_batch task with dependencies on btc tasks
        let task_id = task_tracker
            .create_task(ZKVMInput::L1Batch(input), dependencies)
            .await;
        Ok(task_id)
    }
}
