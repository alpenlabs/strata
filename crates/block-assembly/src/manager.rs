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
use strata_state::{block::L2BlockBundle, header::L2BlockHeader};
use strata_status::StatusChannel;

use crate::{BlockCompletionData, BlockTemplate, BlockTemplateFull, Error};

#[derive(Debug)]
pub struct BlockTemplateManager<D, E> {
    pub(crate) params: Arc<Params>,
    pub(crate) database: Arc<D>,
    pub(crate) engine: Arc<E>,
    pub(crate) status_channel: StatusChannel,
    // TODO: add some form of expiry to purge orphaned items
    pending_templates: HashMap<L2BlockId, BlockTemplateFull>,
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
        }
    }

    pub fn insert_template(&mut self, block_id: L2BlockId, template: BlockTemplateFull) {
        self.pending_templates.insert(block_id, template);
    }

    pub fn get_block_template(&self, block_id: L2BlockId) -> Result<BlockTemplate, Error> {
        self.pending_templates
            .get(&block_id)
            .map(BlockTemplate::from_full_ref)
            .ok_or(Error::UnknownBlockId(block_id))
    }

    pub fn complete_block_template(
        &mut self,
        block_id: L2BlockId,
        completion: BlockCompletionData,
    ) -> Result<L2BlockBundle, Error> {
        match self.pending_templates.entry(block_id) {
            Vacant(entry) => Err(Error::UnknownBlockId(entry.into_key())),
            Occupied(entry) => {
                if !is_completion_data_valid(
                    self.params.rollup(),
                    entry.get().header(),
                    &completion,
                ) {
                    Err(Error::InvalidSignature(block_id, *completion.signature()))
                } else {
                    let template = entry.remove();
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
