use std::sync::Arc;

use bitcoin::params::MAINNET;
use strata_btcio::{
    reader::query::get_verification_state,
    rpc::{traits::Reader, BitcoinClient},
};
use strata_db::traits::ProofDatabase;
use strata_primitives::proof::{ProofId, ProofKey, ProofZkVmHost};
use strata_proofimpl_l1_batch::{L1BatchProofInput, L1BatchProver};
use strata_rocksdb::prover::db::ProofDb;
use strata_zkvm::ZkVmHost;
use tokio::sync::Mutex;

use super::{btc::BtcBlockspaceHandler, ProvingOp};
use crate::{errors::ProvingTaskError, hosts, primitives::vms::ProofVm, task::TaskTracker};

#[derive(Debug, Clone)]
pub struct L1BatchHandler {
    btc_client: Arc<BitcoinClient>,
    btc_blockspace_handler: Arc<BtcBlockspaceHandler>,
}

impl L1BatchHandler {
    pub fn new(
        btc_client: Arc<BitcoinClient>,
        btc_blockspace_handler: Arc<BtcBlockspaceHandler>,
    ) -> Self {
        Self {
            btc_client,
            btc_blockspace_handler,
        }
    }
}

impl ProvingOp for L1BatchHandler {
    type Prover = L1BatchProver;
    type Params = (u64, u64);

    async fn fetch_proof_ids(
        &self,
        params: (u64, u64),
        task_tracker: Arc<Mutex<TaskTracker>>,
        db: &ProofDb,
        hosts: &[ProofZkVmHost],
    ) -> Result<(ProofId, Vec<ProofId>), ProvingTaskError> {
        let (start_height, end_height) = params;

        let len = (end_height - start_height) as usize + 1;
        let mut btc_deps = Vec::with_capacity(len);

        let start_blkid = self.btc_blockspace_handler.get_id(start_height).await?;
        let end_blkid = self.btc_blockspace_handler.get_id(end_height).await?;
        let l1_batch_proof_id = ProofId::L1Batch(start_blkid, end_blkid);

        for height in start_height..=end_height {
            let blkid = self.btc_blockspace_handler.get_id(height).await?;
            let proof_id = ProofId::BtcBlockspace(blkid);
            self.btc_blockspace_handler
                .create_task(height, task_tracker.clone(), db, hosts)
                .await?;
            btc_deps.push(proof_id);
        }

        db.put_proof_deps(l1_batch_proof_id, btc_deps.clone())
            .map_err(ProvingTaskError::DatabaseError)?;

        Ok((l1_batch_proof_id, btc_deps))
    }

    async fn fetch_input(
        &self,
        task_id: &ProofKey,
        db: &ProofDb,
    ) -> Result<L1BatchProofInput, ProvingTaskError> {
        let (start_blkid, _) = match task_id.id() {
            ProofId::L1Batch(start, end) => (*start, end),
            _ => return Err(ProvingTaskError::InvalidInput("L1Batch".to_string())),
        };

        let deps = db
            .get_proof_deps(*task_id.id())
            .map_err(ProvingTaskError::DatabaseError)?
            .ok_or(ProvingTaskError::DependencyNotFound(*task_id))?;

        let mut batch = Vec::new();
        for proof_id in deps {
            let proof_key = ProofKey::new(proof_id, *task_id.host());
            let proof = db
                .get_proof(proof_key)
                .map_err(ProvingTaskError::DatabaseError)?
                .ok_or(ProvingTaskError::ProofNotFound(proof_key))?;
            batch.push(proof);
        }

        let start_height = self
            .btc_client
            .get_block(&start_blkid.into())
            .await
            .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?
            .bip34_block_height()
            .expect("expected bip34 block height");

        let state = get_verification_state(
            self.btc_client.as_ref(),
            start_height,
            &MAINNET.clone().into(),
        )
        .await
        .map_err(|e| ProvingTaskError::RpcError(e.to_string()))?;

        let blockspace_vk = hosts::get_verification_key(&ProofKey::new(
            ProofId::BtcBlockspace(start_blkid),
            *task_id.host(),
        ));

        Ok(L1BatchProofInput {
            batch,
            state,
            blockspace_vk,
        })
    }
}
