use strata_primitives::l2::L2BlockId;
use strata_state::block::L2BlockBundle;
use tokio::sync::{mpsc, oneshot};

use super::{BlockCompletionData, BlockGenerationConfig, BlockTemplate, Error, SharedState};

/// Request to be sent from [`TemplateManagerHandle`] to [`super::worker`].
/// Each also passes a [`oneshot::Sender`] to return the result of the operation.
#[derive(Debug)]
pub enum TemplateManagerRequest {
    /// Build and return a new block template signable by sequencer.
    GenerateBlockTemplate(
        BlockGenerationConfig,
        oneshot::Sender<Result<BlockTemplate, Error>>,
    ),
    /// Provide [`BlockCompletionData`] for an existing template to create
    /// a complete [`L2BlockBundle`]
    CompleteBlockTemplate(
        L2BlockId,
        BlockCompletionData,
        oneshot::Sender<Result<L2BlockBundle, Error>>,
    ),
}

#[derive(Debug, Clone)]
pub struct TemplateManagerHandle {
    tx: mpsc::Sender<TemplateManagerRequest>,
    shared: SharedState,
}

impl TemplateManagerHandle {
    pub fn new(tx: mpsc::Sender<TemplateManagerRequest>, shared: SharedState) -> Self {
        Self { tx, shared }
    }

    async fn request<R>(
        &self,
        build_request: impl FnOnce(oneshot::Sender<Result<R, Error>>) -> TemplateManagerRequest,
    ) -> Result<R, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(build_request(tx))
            .await
            .map_err(|_| Error::RequestChannelClosed)?;

        match rx.await {
            Ok(res) => res,
            // oneshot tx is dropped
            Err(_) => Err(Error::ResponseChannelClosed),
        }
    }

    pub async fn generate_block_template(
        &self,
        config: BlockGenerationConfig,
    ) -> Result<BlockTemplate, Error> {
        // check if pending template exists
        if let Ok(template) = self
            .shared
            .read()
            .await
            .get_pending_block_template_by_parent(config.parent_block_id())
        {
            return Ok(template);
        }

        self.request(|tx| TemplateManagerRequest::GenerateBlockTemplate(config.clone(), tx))
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
        self.shared
            .read()
            .await
            .get_pending_block_template(template_id)
    }
}
