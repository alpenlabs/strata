use std::sync::Arc;

use strata_common::config::Config;
use strata_db::traits::Database;
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::params::Params;
use strata_status::StatusChannel;
use strata_storage::managers::DbManager;
use strata_tasks::TaskManager;
use tokio::sync::{broadcast, mpsc};

use crate::csm_handle::{ClientUpdateNotif, CsmController, CsmMessage};

/// Context relevant for building client components
pub struct BuildContext<D: Database> {
    pub config: Config,
    pub params: Params,
    pub db_manager: DbManager<D>,
}

impl<D: Database> BuildContext<D> {
    pub fn new(config: Config, params: Params, db_manager: DbManager<D>) -> Self {
        Self {
            config,
            params,
            db_manager,
        }
    }
}

/// Context relevant for running client components
pub struct RunContext<D: Database> {
    pub config: Config,
    pub params: Params,
    pub db_manager: DbManager<D>,
    pub task_manager: TaskManager,
    pub status_channel: StatusChannel,
    pub csm_handle: CsmController,
    pub cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
}

/// Context relevant for the main consensus machine
pub struct CsmContext<D: Database + Sync + Send + 'static, E: ExecEngineCtl> {
    pub config: Config,
    pub params: Params,
    pub db_manager: DbManager<D>,
    pub task_manager: TaskManager,
    pub status_channel: StatusChannel,
    pub csm_handle: CsmController,
    pub csm_rx: mpsc::Receiver<CsmMessage>,
    pub database: Arc<D>,
    pub engine: Arc<E>,
}

pub struct ComponentHandle;
