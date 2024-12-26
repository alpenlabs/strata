use std::sync::Arc;

use jsonrpsee::{
    core::{client::ClientT, params::ArrayParams},
    http_client::HttpClient,
    rpc_params,
};
use strata_db::traits::ProofDatabase;
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_types::{HexBytes, ProofKey};
use tracing::info;

/// Submits checkpoint proof to the sequencer.
pub async fn submit_checkpoint_proof(
    checkpoint_index: u64,
    sequencer_client: &HttpClient,
    proof_key: ProofKey,
    proof_db: Arc<ProofDb>,
) -> anyhow::Result<()> {
    let proof = proof_db.get_proof(proof_key).unwrap().unwrap();
    let proof_bytes = HexBytes::from(proof.proof().as_bytes());

    info!(
        "Sending checkpoint proof: {:?} ckp id: {:?} to the sequencer",
        proof_key, checkpoint_index
    );

    sequencer_client
        .request::<(), ArrayParams>(
            "strataadmin_submitCheckpointProof",
            rpc_params![checkpoint_index, proof_bytes],
        )
        .await
        .map_err(|e| anyhow::anyhow!("Failed to submit checkpoint proof: {:?}", e))
}
