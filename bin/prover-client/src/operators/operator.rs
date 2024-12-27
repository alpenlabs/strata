use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_btcio::rpc::BitcoinClient;
use strata_primitives::{params::RollupParams, proof::ProofContext};
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::ProofKey;

use super::{
    btc::BtcBlockspaceOperator, checkpoint::CheckpointOperator, cl_agg::ClAggOperator,
    cl_stf::ClStfOperator, evm_ee::EvmEeOperator, l1_batch::L1BatchOperator, ProvingOp,
};
use crate::{
    errors::ProvingTaskError,
    hosts::{resolve_host, ZkVmHostInstance},
};

/// A struct that manages various proof operators, each corresponding to a distinct proof type.
///
/// The `ProofOperator` provides initialization, accessors, and methods for orchestrating
/// proof generation and processing. It is designed to encapsulate multiple operators
/// implementing the `ProvingOp` trait while handling the trait's lack of object safety
/// by organizing operations through this struct.
#[derive(Debug, Clone)]
pub struct ProofOperator {
    btc_blockspace_operator: BtcBlockspaceOperator,
    l1_batch_operator: L1BatchOperator,
    evm_ee_operator: EvmEeOperator,
    cl_stf_operator: ClStfOperator,
    cl_agg_operator: ClAggOperator,
    checkpoint_operator: CheckpointOperator,
}

impl ProofOperator {
    /// Creates a new instance of `ProofOperator` with the provided proof operators.
    pub fn new(
        btc_blockspace_operator: BtcBlockspaceOperator,
        l1_batch_operator: L1BatchOperator,
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

    /// Initializes a `ProofOperator` by creating and configuring the underlying proof operators.
    pub fn init(
        btc_client: BitcoinClient,
        evm_ee_client: HttpClient,
        cl_client: HttpClient,
        rollup_params: RollupParams,
    ) -> Self {
        let btc_client = Arc::new(btc_client);
        let rollup_params = Arc::new(rollup_params);

        // Create each operator using the respective clients.
        let btc_blockspace_operator =
            BtcBlockspaceOperator::new(btc_client.clone(), rollup_params.clone());
        let l1_batch_operator = L1BatchOperator::new(
            btc_client.clone(),
            Arc::new(btc_blockspace_operator.clone()),
        );
        let evm_ee_operator = EvmEeOperator::new(evm_ee_client.clone());
        let cl_stf_operator = ClStfOperator::new(
            cl_client.clone(),
            Arc::new(evm_ee_operator.clone()),
            rollup_params.clone(),
        );
        let cl_agg_operator = ClAggOperator::new(Arc::new(cl_stf_operator.clone()));
        let checkpoint_operator = CheckpointOperator::new(
            cl_client.clone(),
            Arc::new(l1_batch_operator.clone()),
            Arc::new(cl_agg_operator.clone()),
            rollup_params.clone(),
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

    /// Asynchronously generates a proof using the specified operator and host environment.
    pub async fn prove(
        operator: &impl ProvingOp,
        proof_key: &ProofKey,
        db: &ProofDb,
        host: ZkVmHostInstance,
    ) -> Result<(), ProvingTaskError> {
        match host {
            ZkVmHostInstance::Native(host) => operator.prove(proof_key, db, &host).await,

            #[cfg(feature = "sp1")]
            ZkVmHostInstance::SP1(host) => operator.prove(proof_key, db, host).await,

            #[cfg(feature = "risc0")]
            ZkVmHostInstance::Risc0(host) => operator.prove(proof_key, db, host).await,
        }
    }

    /// Processes a proof generation task by delegating to the appropriate proof operator.
    pub async fn process_proof(
        &self,
        proof_key: &ProofKey,
        db: &ProofDb,
    ) -> Result<(), ProvingTaskError> {
        let host = resolve_host(proof_key);

        match proof_key.context() {
            ProofContext::BtcBlockspace(_) => {
                Self::prove(&self.btc_blockspace_operator, proof_key, db, host).await
            }
            ProofContext::L1Batch(_, _) => {
                Self::prove(&self.l1_batch_operator, proof_key, db, host).await
            }
            ProofContext::EvmEeStf(_, _) => {
                Self::prove(&self.evm_ee_operator, proof_key, db, host).await
            }
            ProofContext::ClStf(_) => Self::prove(&self.cl_stf_operator, proof_key, db, host).await,
            ProofContext::ClAgg(_, _) => {
                Self::prove(&self.cl_agg_operator, proof_key, db, host).await
            }
            ProofContext::Checkpoint(_) => {
                Self::prove(&self.checkpoint_operator, proof_key, db, host).await
            }
        }
    }

    /// Returns a reference to the [`BtcBlockspaceOperator`].
    pub fn btc_operator(&self) -> &BtcBlockspaceOperator {
        &self.btc_blockspace_operator
    }

    /// Returns a reference to the [`L1BatchOperator`]
    pub fn l1_batch_operator(&self) -> &L1BatchOperator {
        &self.l1_batch_operator
    }

    /// Returns a reference to the [`EvmEeOperator`].
    pub fn evm_ee_operator(&self) -> &EvmEeOperator {
        &self.evm_ee_operator
    }

    /// Returns a reference to the [`ClStfOperator`].
    pub fn cl_stf_operator(&self) -> &ClStfOperator {
        &self.cl_stf_operator
    }

    /// Returns a reference to the [`ClAggOperator`].
    pub fn cl_agg_operator(&self) -> &ClAggOperator {
        &self.cl_agg_operator
    }

    /// Returns a reference to the [`CheckpointOperator`].
    pub fn checkpoint_operator(&self) -> &CheckpointOperator {
        &self.checkpoint_operator
    }
}
