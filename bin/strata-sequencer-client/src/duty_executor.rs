use std::{collections::HashSet, sync::Arc};

use strata_primitives::buf::Buf32;
use strata_rpc_api::StrataSequencerApiClient;
use strata_rpc_types::HexBytes64;
use strata_sequencer::{
    block_template::{BlockCompletionData, BlockGenerationConfig},
    duty::types::{BatchCheckpointDuty, BlockSigningDuty, Duty, IdentityData},
    utils::now_millis,
};
use thiserror::Error;
use tokio::{runtime::Handle, select, sync::mpsc};
use tracing::{debug, error};

use crate::helpers::{sign_checkpoint, sign_header};

#[derive(Debug, Error)]
enum DutyExecError {
    #[error("failed generating template: {0}")]
    GenerateTemplate(jsonrpsee::core::ClientError),
    #[error("failed completing template: {0}")]
    CompleteTemplate(jsonrpsee::core::ClientError),
    #[error("failed submitting checkpoint signature: {0}")]
    CompleteCheckpoint(jsonrpsee::core::ClientError),
}

pub(crate) async fn duty_executor_worker<R>(
    rpc: Arc<R>,
    mut duty_rx: mpsc::Receiver<Duty>,
    handle: Handle,
    idata: IdentityData,
) -> anyhow::Result<()>
where
    R: StrataSequencerApiClient + Send + Sync + 'static,
{
    // Keep track of seen duties to avoid processing the same duty multiple times.
    // Does not need to be persisted, as new duties are generated based on current chain state.
    let mut seen_duties = HashSet::new();
    let (failed_duties_tx, mut failed_duties_rx) = mpsc::channel::<Buf32>(8);

    loop {
        select! {
            duty = duty_rx.recv() => {
                if let Some(duty) = duty {
                    let duty_id = duty.id();
                    if seen_duties.contains(&duty_id) {
                        debug!("skipping already seen duty: {:?}", duty);
                        continue;
                    }
                    seen_duties.insert(duty.id());
                    handle.spawn(handle_duty(rpc.clone(), duty, idata.clone(), failed_duties_tx.clone()));
                } else {
                    // tx is closed, we are done
                    return Ok(());
                }
            }
            failed_duty = failed_duties_rx.recv() => {
                if let Some(failed_duty_id) = failed_duty {
                    // remove from seen duties, so we can retry if the duty is seen again
                    seen_duties.remove(&failed_duty_id);
                }
            }
        }
    }
}

async fn handle_duty<R>(
    rpc: Arc<R>,
    duty: Duty,
    idata: IdentityData,
    failed_duties_tx: mpsc::Sender<Buf32>,
) where
    R: StrataSequencerApiClient + Send + Sync,
{
    let duty_result = match duty.clone() {
        Duty::SignBlock(duty) => handle_sign_block_duty(rpc, duty, idata).await,
        Duty::CommitBatch(duty) => handle_commit_batch_duty(rpc, duty, idata).await,
    };

    if let Err(e) = duty_result {
        error!(?duty, "duty failed: {}", e);
        let _ = failed_duties_tx.send(duty.id()).await;
    }
}

async fn handle_sign_block_duty<R>(
    rpc: Arc<R>,
    duty: BlockSigningDuty,
    idata: IdentityData,
) -> Result<(), DutyExecError>
where
    R: StrataSequencerApiClient + Send + Sync,
{
    if now_millis() < duty.target_ts() {
        // wait until target time
        // TODO: ensure duration is within some bounds
        tokio::time::sleep(tokio::time::Duration::from_millis(
            duty.target_ts() - now_millis(),
        ))
        .await;
    }

    // should this keep track of previously signed slots and dont sign conflicting blocks ?
    let template = rpc
        .get_block_template(BlockGenerationConfig::from_parent_block_id(duty.parent()))
        .await
        .map_err(DutyExecError::GenerateTemplate)?;

    let signature = sign_header(template.header(), &idata.key);
    let completion = BlockCompletionData::from_signature(signature);

    rpc.complete_block_template(template.template_id(), completion)
        .await
        .map_err(DutyExecError::CompleteTemplate)?;

    Ok(())
}

async fn handle_commit_batch_duty<R>(
    rpc: Arc<R>,
    duty: BatchCheckpointDuty,
    idata: IdentityData,
) -> Result<(), DutyExecError>
where
    R: StrataSequencerApiClient + Send + Sync,
{
    let sig = sign_checkpoint(duty.checkpoint(), &idata.key);

    rpc.complete_checkpoint_signature(duty.checkpoint().batch_info().idx(), HexBytes64(sig.0))
        .await
        .map_err(DutyExecError::CompleteCheckpoint)?;

    Ok(())
}