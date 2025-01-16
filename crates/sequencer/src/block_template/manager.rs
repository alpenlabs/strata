use std::{
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        HashMap,
    },
    sync::Arc,
};

use strata_crypto::verify_schnorr_sig;
use strata_db::traits::Database;
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::{
    block_credential::CredRule,
    l2::L2BlockId,
    params::{Params, RollupParams},
};
use strata_state::{
    block::L2BlockBundle,
    header::{L2BlockHeader, L2Header},
};
use strata_status::StatusChannel;
use tracing::warn;

use crate::block_template::{BlockCompletionData, BlockTemplate, Error, FullBlockTemplate};

#[derive(Debug)]
pub struct BlockTemplateManager<D, E> {
    pub(crate) params: Arc<Params>,
    pub(crate) database: Arc<D>,
    pub(crate) engine: Arc<E>,
    pub(crate) status_channel: StatusChannel,
    // TODO: add some form of expiry to purge orphaned items
    /// templateid -> template
    pending_templates: HashMap<L2BlockId, FullBlockTemplate>,
    /// parent blockid -> templateid
    pending_by_parent: HashMap<L2BlockId, L2BlockId>,
}

impl<D, E> BlockTemplateManager<D, E>
where
    D: Database,
    E: ExecEngineCtl,
{
    pub fn new(
        params: Arc<Params>,
        database: Arc<D>,
        engine: Arc<E>,
        status_channel: StatusChannel,
    ) -> Self {
        Self {
            params,
            database,
            engine,
            status_channel,
            pending_templates: HashMap::new(),
            pending_by_parent: HashMap::new(),
        }
    }

    pub(crate) fn insert_template(&mut self, template_id: L2BlockId, template: FullBlockTemplate) {
        let parent_blockid = *template.header().parent();
        if let Some(_existing) = self.pending_templates.insert(template_id, template) {
            warn!("existing pending block template overwritten: {template_id}");
        }
        self.pending_by_parent.insert(parent_blockid, template_id);
    }

    pub(crate) fn get_pending_block_template(
        &self,
        template_id: L2BlockId,
    ) -> Result<BlockTemplate, Error> {
        self.pending_templates
            .get(&template_id)
            .map(BlockTemplate::from_full_ref)
            .ok_or(Error::UnknownTemplateId(template_id))
    }

    pub(crate) fn get_pending_block_template_by_parent(
        &self,
        parent_block_id: L2BlockId,
    ) -> Result<BlockTemplate, Error> {
        let block_id = self
            .pending_by_parent
            .get(&parent_block_id)
            .ok_or(Error::UnknownTemplateId(parent_block_id))?;

        self.get_pending_block_template(*block_id)
    }

    pub(crate) fn complete_block_template(
        &mut self,
        template_id: L2BlockId,
        completion: BlockCompletionData,
    ) -> Result<L2BlockBundle, Error> {
        match self.pending_templates.entry(template_id) {
            Vacant(entry) => Err(Error::UnknownTemplateId(entry.into_key())),
            Occupied(entry) => {
                if !is_completion_data_valid(
                    self.params.rollup(),
                    entry.get().header(),
                    &completion,
                ) {
                    Err(Error::InvalidSignature(
                        template_id,
                        *completion.signature(),
                    ))
                } else {
                    let template = entry.remove();
                    let parent = template.header().parent();
                    self.pending_by_parent.remove(parent);
                    Ok(template.complete_block_template(completion))
                }
            }
        }
    }
}

fn is_completion_data_valid(
    rollup_params: &RollupParams,
    header: &L2BlockHeader,
    completion: &BlockCompletionData,
) -> bool {
    let sighash = header.get_sighash();
    match &rollup_params.cred_rule {
        CredRule::Unchecked => true,
        CredRule::SchnorrKey(pubkey) => {
            verify_schnorr_sig(completion.signature(), &sighash, pubkey)
        }
    }
}
