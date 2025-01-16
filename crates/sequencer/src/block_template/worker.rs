use strata_db::traits::{Database, L2BlockDatabase};
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::params::Params;
use strata_state::header::L2Header;
use strata_status::StatusChannel;
use strata_tasks::ShutdownGuard;
use tokio::sync::mpsc;
use tracing::warn;

use crate::{
    block_template::{
        prepare_block, BlockGenerationConfig, BlockTemplate, BlockTemplateManager, Error,
        FullBlockTemplate, TemplateManagerRequest,
    },
    utils::now_millis,
};

/// Worker task for block template manager.
pub fn template_manager_worker<D, E>(
    shutdown: ShutdownGuard,
    mut manager: BlockTemplateManager<D, E>,
    mut rx: mpsc::Receiver<TemplateManagerRequest>,
) -> anyhow::Result<()>
where
    D: Database,
    E: ExecEngineCtl,
{
    while let Some(request) = rx.blocking_recv() {
        match request {
            TemplateManagerRequest::GenerateBlockTemplate(config, sender) => {
                if sender
                    .send(generate_block_template(&mut manager, config))
                    .is_err()
                {
                    warn!("failed sending GenerateBlockTemplate result");
                }
            }
            TemplateManagerRequest::CompleteBlockTemplate(template_id, completion, sender) => {
                if sender
                    .send(manager.complete_block_template(template_id, completion))
                    .is_err()
                {
                    warn!(?template_id, "failed sending CompleteBlockTemplate result");
                }
            }
            TemplateManagerRequest::GetBlockTemplate(template_id, sender) => {
                if sender
                    .send(manager.get_pending_block_template(template_id))
                    .is_err()
                {
                    warn!(?template_id, "failed sending GetBlockTemplate result")
                }
            }
        };

        if shutdown.should_shutdown() {
            break;
        }
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
    if let Ok(template) = manager.get_pending_block_template_by_parent(config.parent_block_id()) {
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

    let template_id = full_template.get_blockid();

    manager.insert_template(template_id, full_template);

    Ok(template)
}

fn generate_block_template_inner<D: Database, E: ExecEngineCtl>(
    config: BlockGenerationConfig,
    params: &Params,
    database: &D,
    engine: &E,
    status_channel: &StatusChannel,
) -> Result<FullBlockTemplate, Error> {
    // get parent block
    let parent_block_id = config.parent_block_id();
    let l2db = database.l2_db();
    let parent = l2db
        .get_block_data(parent_block_id)?
        .ok_or(Error::UnknownTemplateId(parent_block_id))?;

    let parent_ts = parent.header().timestamp();

    // next slot idx
    let slot = parent.header().blockidx() + 1;

    // next block timestamp
    let ts = config.ts().unwrap_or_else(now_millis);

    // maintain min block_time
    if ts < parent_ts + params.rollup().block_time {
        Err(Error::TimestampTooEarly(ts))?;
    }

    // latest l1 view from client state
    let l1_state = status_channel.l1_view();

    let (header, body, accessory) =
        prepare_block(slot, parent, &l1_state, ts, database, engine, params)?;

    Ok(FullBlockTemplate::new(header, body, accessory))
}
