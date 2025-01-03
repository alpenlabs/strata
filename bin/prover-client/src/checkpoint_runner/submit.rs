use std::sync::Arc;

use jsonrpsee::http_client::HttpClient;
use strata_db::traits::ProofDatabase;
use strata_rocksdb::prover::db::ProofDb;
use strata_rpc_api::StrataSequencerApiClient;
use strata_rpc_types::{HexBytes, ProofKey};
use tracing::info;

use super::error::{CheckpointError, CheckpointResult};

/// Submits checkpoint proof to the sequencer.
pub async fn submit_checkpoint_proof(
    checkpoint_index: u64,
    sequencer_client: &HttpClient,
    proof_key: ProofKey,
    proof_db: Arc<ProofDb>,
) -> CheckpointResult<()> {
    let proof = proof_db.get_proof(proof_key).unwrap().unwrap();
    let proof_bytes = HexBytes::from(proof.proof().as_bytes());

    info!(
        "Sending checkpoint proof: {:?} ckp id: {:?} to the sequencer",
        proof_key, checkpoint_index
    );

    sequencer_client
        .submit_checkpoint_proof(checkpoint_index, proof_bytes)
        .await
        .map_err(|e| CheckpointError::SubmitProofError {
            index: checkpoint_index,
            error: e.to_string(),
        })
}
