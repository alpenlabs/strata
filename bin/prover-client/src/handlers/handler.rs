use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_btcio::rpc::BitcoinClient;
use strata_primitives::proof::{ProofId, ProofZkVmHost};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::ProofKey;

use super::{
    btc::BtcBlockspaceHandler, checkpoint::CheckpointHandler, cl_agg::ClAggHandler,
    cl_stf::ClStfHandler, evm_ee::EvmEeHandler, l1_batch::L1BatchHandler, ProvingOp,
};
use crate::errors::ProvingTaskError;

#[derive(Debug, Clone)]
pub struct ProofHandler {
    btc_blockspace_handler: BtcBlockspaceHandler,
    l1_batch_handler: L1BatchHandler,
    evm_ee_handler: EvmEeHandler,
    cl_stf_handler: ClStfHandler,
    cl_agg_handler: ClAggHandler,
    checkpoint_handler: CheckpointHandler,
    vms: Vec<ProofZkVmHost>,
}

impl ProofHandler {
    pub fn new(
        btc_blockspace_handler: BtcBlockspaceHandler,
        l1_batch_handler: L1BatchHandler,
        evm_ee_handler: EvmEeHandler,
        cl_stf_handler: ClStfHandler,
        cl_agg_handler: ClAggHandler,
        checkpoint_handler: CheckpointHandler,
        vms: Vec<ProofZkVmHost>,
    ) -> Self {
        Self {
            btc_blockspace_handler,
            l1_batch_handler,
            evm_ee_handler,
            cl_stf_handler,
            cl_agg_handler,
            checkpoint_handler,
            vms,
        }
    }

    pub fn init(
        btc_client: BitcoinClient,
        evm_ee_client: HttpClient,
        cl_client: HttpClient,
    ) -> Self {
        let btc_client = Arc::new(btc_client);
        let btc_blockspace_handler = BtcBlockspaceHandler::new(btc_client.clone());
        let l1_batch_handler =
            L1BatchHandler::new(btc_client.clone(), Arc::new(btc_blockspace_handler.clone()));
        let evm_ee_handler = EvmEeHandler::new(evm_ee_client.clone());
        let cl_stf_handler = ClStfHandler::new(cl_client.clone(), Arc::new(evm_ee_handler.clone()));
        let cl_agg_handler = ClAggHandler::new(Arc::new(cl_stf_handler.clone()));
        let checkpoint_handler = CheckpointHandler::new(
            cl_client.clone(),
            Arc::new(l1_batch_handler.clone()),
            Arc::new(cl_agg_handler.clone()),
        );

        let mut vms = vec![];

        #[cfg(feature = "sp1")]
        {
            vms.push(ProofZkVmHost::SP1);
        }

        #[cfg(feature = "risc0")]
        {
            vms.push(ProofZkVmHost::Risc0);
        }

        #[cfg(all(not(feature = "risc0"), not(feature = "sp1")))]
        {
            vms.push(ProofZkVmHost::Native);
        }

        ProofHandler::new(
            btc_blockspace_handler,
            l1_batch_handler,
            evm_ee_handler,
            cl_stf_handler,
            cl_agg_handler,
            checkpoint_handler,
            vms,
        )
    }

    pub async fn prove(&self, proof_key: &ProofKey, db: &ProofDb) -> Result<(), ProvingTaskError> {
        match proof_key.id() {
            ProofId::BtcBlockspace(_) => self.btc_blockspace_handler.prove(proof_key, db).await,
            ProofId::L1Batch(_, _) => self.l1_batch_handler.prove(proof_key, db).await,
            ProofId::EvmEeStf(_) => self.evm_ee_handler.prove(proof_key, db).await,
            ProofId::ClStf(_) => self.cl_stf_handler.prove(proof_key, db).await,
            ProofId::ClAgg(_, _) => self.cl_agg_handler.prove(proof_key, db).await,
            ProofId::Checkpoint(_) => self.checkpoint_handler.prove(proof_key, db).await,
        }
    }

    pub fn btc_handler(&self) -> &BtcBlockspaceHandler {
        &self.btc_blockspace_handler
    }

    pub fn l1_batch_handler(&self) -> &L1BatchHandler {
        &self.l1_batch_handler
    }

    pub fn evm_ee_handler(&self) -> &EvmEeHandler {
        &self.evm_ee_handler
    }

    pub fn cl_stf_handler(&self) -> &ClStfHandler {
        &self.cl_stf_handler
    }

    pub fn cl_agg_handler(&self) -> &ClAggHandler {
        &self.cl_agg_handler
    }

    pub fn checkpoint_handler(&self) -> &CheckpointHandler {
        &self.checkpoint_handler
    }

    pub fn vms(&self) -> &[ProofZkVmHost] {
        &self.vms
    }
}
