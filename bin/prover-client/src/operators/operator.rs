use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_btcio::rpc::BitcoinClient;
use strata_primitives::proof::ProofContext;
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::ProofKey;

use super::{
    btc::BtcBlockspaceOperator, checkpoint::CheckpointOperator, cl_agg::ClAggOperator,
    cl_stf::ClStfOperator, evm_ee::EvmEeOperator, l1_batch::L1BatchController, ProvingOp,
};
use crate::errors::ProvingTaskError;

#[derive(Debug, Clone)]
pub struct ProofOperator {
    btc_blockspace_operator: BtcBlockspaceOperator,
    l1_batch_operator: L1BatchController,
    evm_ee_operator: EvmEeOperator,
    cl_stf_operator: ClStfOperator,
    cl_agg_operator: ClAggOperator,
    checkpoint_operator: CheckpointOperator,
}

impl ProofOperator {
    pub fn new(
        btc_blockspace_operator: BtcBlockspaceOperator,
        l1_batch_operator: L1BatchController,
        evm_ee_operator: EvmEeOperator,
        cl_stf_operator: ClStfOperator,
        cl_agg_operator: ClAggOperator,
        checkpoint_operator: CheckpointOperator,
    ) -> Self {
        Self {
            btc_blockspace_operator,
            l1_batch_operator,
            evm_ee_operator,
            cl_stf_operator,
            cl_agg_operator,
            checkpoint_operator,
        }
    }

    pub fn init(
        btc_client: BitcoinClient,
        evm_ee_client: HttpClient,
        cl_client: HttpClient,
    ) -> Self {
        let btc_client = Arc::new(btc_client);
        let btc_blockspace_operator = BtcBlockspaceOperator::new(btc_client.clone());
        let l1_batch_operator = L1BatchController::new(
            btc_client.clone(),
            Arc::new(btc_blockspace_operator.clone()),
        );
        let evm_ee_operator = EvmEeOperator::new(evm_ee_client.clone());
        let cl_stf_operator =
            ClStfOperator::new(cl_client.clone(), Arc::new(evm_ee_operator.clone()));
        let cl_agg_operator = ClAggOperator::new(Arc::new(cl_stf_operator.clone()));
        let checkpoint_operator = CheckpointOperator::new(
            cl_client.clone(),
            Arc::new(l1_batch_operator.clone()),
            Arc::new(cl_agg_operator.clone()),
        );

        ProofOperator::new(
            btc_blockspace_operator,
            l1_batch_operator,
            evm_ee_operator,
            cl_stf_operator,
            cl_agg_operator,
            checkpoint_operator,
        )
    }

    pub async fn prove(&self, proof_key: &ProofKey, db: &ProofDb) -> Result<(), ProvingTaskError> {
        match proof_key.context() {
            ProofContext::BtcBlockspace(_) => {
                self.btc_blockspace_operator.prove(proof_key, db).await
            }
            ProofContext::L1Batch(_, _) => self.l1_batch_operator.prove(proof_key, db).await,
            ProofContext::EvmEeStf(_) => self.evm_ee_operator.prove(proof_key, db).await,
            ProofContext::ClStf(_) => self.cl_stf_operator.prove(proof_key, db).await,
            ProofContext::ClAgg(_, _) => self.cl_agg_operator.prove(proof_key, db).await,
            ProofContext::Checkpoint(_) => self.checkpoint_operator.prove(proof_key, db).await,
        }
    }

    pub fn btc_operator(&self) -> &BtcBlockspaceOperator {
        &self.btc_blockspace_operator
    }

    pub fn l1_batch_operator(&self) -> &L1BatchController {
        &self.l1_batch_operator
    }

    pub fn evm_ee_operator(&self) -> &EvmEeOperator {
        &self.evm_ee_operator
    }

    pub fn cl_stf_operator(&self) -> &ClStfOperator {
        &self.cl_stf_operator
    }

    pub fn cl_agg_operator(&self) -> &ClAggOperator {
        &self.cl_agg_operator
    }

    pub fn checkpoint_operator(&self) -> &CheckpointOperator {
        &self.checkpoint_operator
    }
}
