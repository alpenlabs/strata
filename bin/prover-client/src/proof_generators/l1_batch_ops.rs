use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use bitcoin::params::MAINNET;
use strata_btcio::{reader::query::get_verification_state, rpc::BitcoinClient};
use strata_proofimpl_l1_batch::L1BatchProver;
use strata_rocksdb::prover::db::ProverDB;
use strata_state::l1::HeaderVerificationState;
use uuid::Uuid;

use super::{btc_ops::BtcBlockspaceProofGenerator, ProofGenerator};
use crate::{
    errors::{ProvingTaskError, ProvingTaskType},
    primitives::prover_input::{ProofWithVkey, ZkVmInput},
    state::ProvingInfo,
    task::TaskTracker,
    task2::TaskTracker2,
};

/// Operations required for BTC block proving tasks.
#[derive(Debug, Clone)]
pub struct L1BatchProofGenerator {
    btc_dispatcher: Arc<BtcBlockspaceProofGenerator>,
    btc_client: Arc<BitcoinClient>,
}

impl L1BatchProofGenerator {
    /// Creates a new BTC operations instance.
    pub fn new(
        btc_dispatcher: Arc<BtcBlockspaceProofGenerator>,
        btc_client: Arc<BitcoinClient>,
    ) -> Self {
        Self {
            btc_dispatcher,
            btc_client,
        }
    }
}

type L1BatchId = (u64, u64);

#[derive(Debug, Clone)]
pub struct L1BatchIntermediateInput {
    pub btc_task_ids: HashMap<Uuid, u64>,
}

impl ProofGenerator for L1BatchProofGenerator {
    // Range of l1 blocks
    type Prover = L1BatchProver;
    type Id = L1BatchId;

    async fn create_task(
        &self,
        id: L1BatchId,
        db: ProverDB,
        task_tracker: Arc<TaskTracker2>,
    ) -> Result<Uuid, ProvingTaskError> {
        let mut dependencies = vec![];
        let mut btc_task_ids = HashMap::new();

        // Create btc tasks for each block in the range
        let (start, end) = id;
        for btc_block_idx in start..=end {
            let btc_task_id = self
                .btc_dispatcher
                .create_task(btc_block_idx, db, task_tracker.clone())
                .await
                .map_err(|e| ProvingTaskError::DependencyTaskCreation(e.to_string()))?;
            dependencies.push(btc_task_id);
            btc_task_ids.insert(btc_task_id, btc_block_idx);
        }

        let intermediate_input = L1BatchIntermediateInput { btc_task_ids };
        let info = ProvingInfo::L1Batch(self.clone(), intermediate_input);
        // let status

        // Create the l1_batch task with dependencies on btc tasks
        let task_id = task_tracker.create_task().await;
        Ok(task_id)
    }

    // async fn fetch_input(&self) -> Result<Self::Input, anyhow::Error> {
    //     let st_height = btc_block_range.0;
    //     let header_verification_state =
    //         get_verification_state(self.btc_client.as_ref(), st_height, &MAINNET.clone().into())
    //             .await?;

    //     let input: Self::Input = L1BatchInput {
    //         btc_block_range,
    //         btc_task_ids: HashMap::new(),
    //         proofs: HashMap::new(),
    //         header_verification_state,
    //     };
    //     Ok(input)
    // }
}
