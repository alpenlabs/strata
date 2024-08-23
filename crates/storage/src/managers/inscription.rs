use std::sync::Arc;

use alpen_express_db::types::BlobEntry;
use alpen_express_state::da_blob::BlobIntent;
use tokio::sync::mpsc::Sender;

use crate::ops::inscription::InscriptionDataOps;

pub struct InscriptionManager {
    ops: Arc<InscriptionDataOps>,
    signer_tx: Sender<u64>,
}

impl InscriptionManager {
    pub fn new(ops: Arc<InscriptionDataOps>, signer_tx: Sender<u64>) -> Self {
        Self { ops, signer_tx }
    }

    pub fn ops(&self) -> &InscriptionDataOps {
        &self.ops
    }

    pub fn submit_intent(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());

        // Write to db and if not already exisging, notify signer about the new entry
        // if let Some(idx) = store_entry(*intent.commitment(), entry, self.db.clone())? {
        if let Some(idx) = self
            .ops
            .put_blob_entry_blocking(*intent.commitment(), entry)?
        {
            self.signer_tx.blocking_send(idx)?;
        } // None means duplicate intent
        Ok(())
    }

    pub async fn submit_intent_async(&self, intent: BlobIntent) -> anyhow::Result<()> {
        // TODO: check for intent dest ??
        let entry = BlobEntry::new_unsigned(intent.payload().to_vec());

        // Write to db and if not already exisging, notify signer about the new entry
        if let Some(idx) = self
            .ops
            .put_blob_entry_async(*intent.commitment(), entry)
            .await?
        {
            self.signer_tx.send(idx).await?;
        } // None means duplicate intent
        Ok(())
    }
}
