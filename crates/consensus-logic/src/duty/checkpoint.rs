use std::{sync::Arc, thread, time::Duration};

use alpen_express_db::types::{CheckpointEntry, CheckpointProvingStatus};
use alpen_express_primitives::params::{Params, ProofPublishMode};
use alpen_express_state::batch::BatchCheckpoint;
use tracing::*;

use super::types::BatchCheckpointDuty;
use crate::{checkpoint::CheckpointHandle, errors::Error};

pub(crate) fn check_and_get_batch_checkpoint(
    duty: &BatchCheckpointDuty,
    checkpt_handle: Arc<CheckpointHandle>,
    pool: threadpool::ThreadPool,
    params: &Params,
) -> Result<BatchCheckpoint, Error> {
    let idx = duty.idx();

    debug!(%idx, "checking for checkpoint in db");
    // If there's no entry in db, create a pending entry and wait until proof is ready
    match checkpt_handle.get_checkpoint_blocking(idx)? {
        // There's no entry in the database, create one so that the prover manager can query the
        // checkpoint info to create proofs for next
        None => {
            debug!(%idx, "Checkpoint not found, creating pending checkpoint");
            let entry = CheckpointEntry::new_pending_proof(duty.checkpoint().clone());
            checkpt_handle.put_checkpoint_blocking(idx, entry)?;
        }
        // There's an entry. If status is ProofCreated, return it else we need to wait for prover to
        // submit proofs.
        Some(entry) => match entry.proving_status {
            CheckpointProvingStatus::PendingProof => {
                // Do nothing, wait for broadcast msg below
            }
            _ => {
                return Ok(entry.into());
            }
        },
    }
    debug!(%idx, "Waiting for checkpoint proof to be posted");

    if let ProofPublishMode::Timeout(timeout) = params.rollup().proof_publish_mode {
        spawn_proof_timeout(idx, checkpt_handle.clone(), timeout, pool);
    }

    let chidx = checkpt_handle
        .subscribe()
        .blocking_recv()
        .map_err(|e| Error::Other(e.to_string()))?;

    debug!(%idx, %chidx, "Received proof from rpc");

    if chidx != idx {
        warn!(received = %chidx, expected = %idx, "Received different checkpoint idx than expected");
        return Err(Error::Other(
            "Unexpected checkpoint idx received from broadcast channel".to_string(),
        ));
    }

    match checkpt_handle.get_checkpoint_blocking(idx)? {
        None => {
            warn!(%idx, "Expected checkpoint to be present in db");
            Err(Error::Other(
                "Expected checkpoint to be present in db".to_string(),
            ))
        }
        Some(entry) if entry.proving_status == CheckpointProvingStatus::PendingProof => {
            warn!(%idx, "Expected checkpoint proof to be ready");
            Err(Error::Other(
                "Expected checkpoint proof to be ready".to_string(),
            ))
        }
        Some(entry) => Ok(entry.into()),
    }
}
fn spawn_proof_timeout(
    idx: u64,
    checkpt_handle: Arc<CheckpointHandle>,
    timeout: u64,
    pool: threadpool::ThreadPool,
) {
    pool.execute(move || {
        // Sleep.
        debug!(%idx, "Starting timeout for proof");
        thread::sleep(Duration::from_secs(timeout));
        debug!(%idx, "Timeout exceeded");

        // Now update and send. Doesn't matter if the receiver is already closed. It means the proof
        // was submitted in time

        match checkpt_handle.get_checkpoint_blocking(idx) {
            Ok(Some(mut entry)) => {
                if entry.proving_status != CheckpointProvingStatus::PendingProof {
                    warn!("Got request for already ready proof");
                    return;
                }
                debug!(%idx, "Proof is pending, setting proof ready");

                entry.proving_status = CheckpointProvingStatus::ProofReady;
                if let Err(e) = checkpt_handle.put_checkpoint_and_notify_blocking(idx, entry) {
                    warn!(?e, "Error updating checkpoint after timeout");
                }
                debug!(%idx, "Successfully submitted proof after timeout");
            }
            Ok(None) => {
                error!(%idx, "Expected checkpoint not found in db");
            }
            Err(e) => {
                error!(%idx, ?e, "DB error occurred while fetching checkpoint ");
            }
        }
    });
}
