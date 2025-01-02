use std::sync::Arc;

use strata_rpc_api::StrataSequencerApiClient;
use strata_sequencer::{
    block_template::{BlockCompletionData, BlockGenerationConfig},
    types::{BlockSigningDuty, Duty, IdentityData},
};
use thiserror::Error;
use tokio::{runtime::Handle, sync::mpsc};
use tracing::error;

use crate::helpers::sign_header;

#[derive(Debug, Error)]
enum DutyExecError {
    #[error("failed generating template: {0}")]
    GenerateTemplate(jsonrpsee::core::ClientError),
    #[error("failed completing template: {0}")]
    CompleteTemplate(jsonrpsee::core::ClientError),
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
    while let Some(duty) = duty_rx.recv().await {
        handle.spawn(handle_duty(rpc.clone(), duty, idata.clone()));
    }

    Ok(())
}

async fn handle_duty<R>(rpc: Arc<R>, duty: Duty, idata: IdentityData)
where
    R: StrataSequencerApiClient + Send + Sync,
{
    let duty_fut = match duty.clone() {
        Duty::SignBlock(duty) => handle_sign_block_duty(rpc, duty, idata),
        Duty::CommitBatch(_duty) => todo!(),
    };

    if let Err(e) = duty_fut.await {
        error!(?duty, "duty failed: {}", e);
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
