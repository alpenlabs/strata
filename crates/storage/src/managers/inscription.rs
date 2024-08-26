use std::sync::Arc;

use alpen_express_db::types::BlobEntry;
use alpen_express_state::da_blob::BlobIntent;

use crate::ops::inscription::InscriptionDataOps;

pub struct InscriptionManager {
    ops: Arc<InscriptionDataOps>,
}

impl InscriptionManager {
    pub fn new(ops: Arc<InscriptionDataOps>) -> Self {
        Self { ops }
    }

    pub fn ops(&self) -> &InscriptionDataOps {
        &self.ops
    }

    pub fn submit_intent(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        tracing::debug!(?intent, "SUBMIT INTENT");
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());

        Ok(self
            .ops
            .put_blob_entry_blocking(*intent.commitment(), entry)?)
    }

    pub async fn submit_intent_async(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());

        Ok(self
            .ops
            .put_blob_entry_async(*intent.commitment(), entry)
            .await?)
    }
}
