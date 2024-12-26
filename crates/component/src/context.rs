use std::sync::Arc;

use strata_common::config::Config;
use strata_db::traits::Database;
use strata_primitives::params::Params;
use strata_status::StatusChannel;
use strata_storage::managers::DbManager;
use strata_tasks::TaskManager;
use tokio::sync::{broadcast, mpsc};

use crate::csm_handle::{ClientUpdateNotif, CsmController, CsmMessage};

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
    pub cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
}

pub struct CsmContext<D: Database + Sync + Send + 'static> {
    pub config: Config,
    pub params: Params,
    pub db_manager: DbManager,
    pub task_manager: TaskManager,
    pub status_channel: StatusChannel,
    pub csm_handle: CsmController,
    pub csm_rx: mpsc::Receiver<CsmMessage>,
    pub database: Arc<D>,
}

pub struct ComponentHandle;
