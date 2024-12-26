use strata_common::config::Config;
use strata_primitives::params::Params;
use strata_state::sync_event::SyncEvent;
use strata_status::StatusChannel;
use strata_storage::managers::DbManager;
use strata_tasks::TaskManager;

use crate::{CsmHandle, csm_handle::CsmController};

pub struct BuildContext {
    pub config: Config,
    pub params: Params,
    pub db_manager: DbManager,
}

impl BuildContext {
    pub fn new(config: Config, params: Params, db_manager: DbManager) -> Self {
        Self {
            config,
            params,
            db_manager,
        }
    }
}

pub struct RunContext {
    pub config: Config,
    pub params: Params,
    pub db_manager: DbManager,
    pub task_manager: TaskManager,
    pub status_channel: StatusChannel,
    pub csm_handle: CsmController,
}

pub struct ComponentHandle;
