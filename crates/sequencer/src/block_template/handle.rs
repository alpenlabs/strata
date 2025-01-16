use std::{num::NonZeroUsize, sync::Arc};

use lru::LruCache;
use strata_primitives::l2::L2BlockId;
use strata_state::block::L2BlockBundle;
use tokio::sync::{mpsc, oneshot, Mutex};

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
    cache: Arc<Mutex<LruCache<L2BlockId, BlockTemplate>>>,
}

impl TemplateManagerHandle {
    pub fn new(tx: mpsc::Sender<TemplateManagerRequest>, cache_size: NonZeroUsize) -> Self {
        Self {
            tx,
            cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
        }
    }

    async fn request<R>(
        &self,
        build_request: impl FnOnce(oneshot::Sender<Result<R, Error>>) -> TemplateManagerRequest,
    ) -> Result<R, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(build_request(tx)).await.map_err(|_| {
            Error::ChannelError("failed to send request to template manager, worker likely exited")
        })?;

        match rx.await {
            Ok(res) => res,
            // oneshot tx is dropped
            Err(_) => Err(Error::ChannelError("tx dropped, worker likely exited")),
        }
    }

    pub async fn generate_block_template(
        &self,
        config: BlockGenerationConfig,
    ) -> Result<BlockTemplate, Error> {
        let template = self
            .request(|tx| TemplateManagerRequest::GenerateBlockTemplate(config, tx))
            .await?;

        self.cache
            .lock()
            .await
            .push(template.template_id(), template.clone());

        Ok(template)
    }

    pub async fn complete_block_template(
        &self,
        template_id: L2BlockId,
        completion: BlockCompletionData,
    ) -> Result<L2BlockBundle, Error> {
        let bundle = self
            .request(|tx| {
                TemplateManagerRequest::CompleteBlockTemplate(template_id, completion, tx)
            })
            .await?;

        self.cache.lock().await.pop(&template_id);

        Ok(bundle)
    }

    pub async fn get_block_template(&self, template_id: L2BlockId) -> Result<BlockTemplate, Error> {
        if let Some(cached) = self.cache.lock().await.get(&template_id) {
            return Ok(cached.clone());
        }
        let template = self
            .request(|tx| TemplateManagerRequest::GetBlockTemplate(template_id, tx))
            .await?;

        self.cache
            .lock()
            .await
            .push(template.template_id(), template.clone());

        Ok(template)
    }
}
