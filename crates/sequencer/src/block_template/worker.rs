use std::time;

use strata_db::traits::{Database, L2BlockDatabase};
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::params::Params;
use strata_state::header::L2Header;
use strata_status::StatusChannel;
use tokio::sync::mpsc;

use crate::{
    block_assembly::prepare_block,
    block_template::{
        BlockGenerationConfig, BlockTemplate, BlockTemplateFull, BlockTemplateManager, Error,
        TemplateManagerRequest,
    },
};

fn now_millis() -> u64 {
    time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64
}

/// Worker task for block template manager.
pub async fn template_manager_worker<D, E>(
    mut manager: BlockTemplateManager<D, E>,
    mut rx: mpsc::Receiver<TemplateManagerRequest>,
) -> anyhow::Result<()>
where
    D: Database,
    E: ExecEngineCtl,
{
    while let Some(request) = rx.recv().await {
        match request {
            TemplateManagerRequest::GenerateBlockTemplate(config, sender) => {
                let _ = sender.send(generate_block_template(&mut manager, config));
            }
            TemplateManagerRequest::CompleteBlockTemplate(template_id, completion, sender) => {
                let _ = sender.send(manager.complete_block_template(template_id, completion));
            }
            TemplateManagerRequest::GetBlockTemplate(block_id, sender) => {
                let _ = sender.send(manager.get_block_template(block_id));
            }
        };
    }

    Ok(())
}

fn generate_block_template<D, E>(
    manager: &mut BlockTemplateManager<D, E>,
    config: BlockGenerationConfig,
) -> Result<BlockTemplate, Error>
where
    D: Database,
    E: ExecEngineCtl,
{
    // check if we already have pending template for this parent block id
    if let Ok(template) = manager.get_block_template_by_parent(config.parent_block_id()) {
        return Ok(template);
    }

    let full_template = generate_block_template_inner(
        config,
        manager.params.as_ref(),
        manager.database.as_ref(),
        manager.engine.as_ref(),
        &manager.status_channel,
    )?;

    let template = BlockTemplate::from_full_ref(&full_template);

    let block_id = full_template.block_id();

    manager.insert_template(block_id, full_template);

    Ok(template)
}

fn generate_block_template_inner<D: Database, E: ExecEngineCtl>(
    config: BlockGenerationConfig,
    params: &Params,
    database: &D,
    engine: &E,
    status_channel: &StatusChannel,
) -> Result<BlockTemplateFull, Error> {
    // get parent block
    let parent_block_id = config.parent_block_id();
    let l2db = database.l2_db();
    let parent = l2db
        .get_block_data(parent_block_id)?
        .ok_or(Error::UnknownBlockId(parent_block_id))?;

    // next slot idx
    let slot = parent.header().blockidx() + 1;

    // next block timestamp
    let ts = config.ts(now_millis());

    // latest l1 view from client state
    let l1_state = status_channel.l1_view();

    let (header, body, accessory) =
        prepare_block(slot, parent, &l1_state, ts, database, engine, params)?;

    Ok(BlockTemplateFull::new(header, body, accessory))
}
