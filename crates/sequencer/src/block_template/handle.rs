use strata_primitives::l2::L2BlockId;
use strata_state::block::L2BlockBundle;
use tokio::sync::{mpsc, oneshot};

use crate::block_template::{BlockCompletionData, BlockGenerationConfig, BlockTemplate, Error};

#[derive(Debug)]
pub enum TemplateManagerRequest {
    GenerateBlockTemplate(
        BlockGenerationConfig,
        oneshot::Sender<Result<BlockTemplate, Error>>,
    ),
    CompleteBlockTemplate(
        L2BlockId,
        BlockCompletionData,
        oneshot::Sender<Result<L2BlockBundle, Error>>,
    ),
    GetBlockTemplate(L2BlockId, oneshot::Sender<Result<BlockTemplate, Error>>),
}

#[derive(Debug, Clone)]
pub struct TemplateManagerHandle {
    tx: mpsc::Sender<TemplateManagerRequest>,
}

impl TemplateManagerHandle {
    pub fn new(tx: mpsc::Sender<TemplateManagerRequest>) -> Self {
        Self { tx }
    }

    async fn request<R>(
        &self,
        build_request: impl FnOnce(oneshot::Sender<Result<R, Error>>) -> TemplateManagerRequest,
    ) -> Result<R, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(build_request(tx))
            .await
            .map_err(|_| Error::ChannelError("send"))?;

        match rx.await {
            Ok(res) => res,
            Err(_) => Err(Error::ChannelError("oneshot tx dropped")),
        }
    }

    pub async fn generate_block_template(
        &self,
        config: BlockGenerationConfig,
    ) -> Result<BlockTemplate, Error> {
        self.request(|tx| TemplateManagerRequest::GenerateBlockTemplate(config, tx))
            .await
    }

    pub async fn complete_block_template(
        &self,
        template_id: L2BlockId,
        completion: BlockCompletionData,
    ) -> Result<L2BlockBundle, Error> {
        self.request(|tx| {
            TemplateManagerRequest::CompleteBlockTemplate(template_id, completion, tx)
        })
        .await
    }

    pub async fn get_block_template(&self, template_id: L2BlockId) -> Result<BlockTemplate, Error> {
        self.request(|tx| TemplateManagerRequest::GetBlockTemplate(template_id, tx))
            .await
    }
}
