//! High level sync manager which controls core sync tasks and manages sync
//! status.  Exposes handles to interact with fork choice manager and CSM
//! executor and other core sync pipeline tasks.

use std::sync::Arc;

use strata_db::traits::Database;
use strata_eectl::engine::ExecEngineCtl;
use strata_primitives::params::Params;
use strata_status::{StatusRx, StatusTx};
use strata_storage::{managers::checkpoint::CheckpointDbManager, L2BlockManager};
use strata_tasks::TaskExecutor;
use tokio::sync::{broadcast, mpsc};

use crate::{
    csm::{
        ctl::CsmController,
        message::{ClientUpdateNotif, CsmMessage, ForkChoiceMessage},
        worker,
    },
    fork_choice_manager,
};

/// Handle to the core pipeline tasks.
pub struct SyncManager {
    params: Arc<Params>,
    fc_manager_tx: mpsc::Sender<ForkChoiceMessage>,
    csm_controller: Arc<CsmController>,
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    status_tx: Arc<StatusTx>,
    status_rx: Arc<StatusRx>,
}

impl SyncManager {
    pub fn params(&self) -> &Params {
        &self.params
    }

    pub fn get_params(&self) -> Arc<Params> {
        self.params.clone()
    }

    /// Gets a ref to the CSM controller.
    pub fn csm_controller(&self) -> &CsmController {
        &self.csm_controller
    }

    /// Gets a clone of the CSM controller.
    pub fn get_csm_ctl(&self) -> Arc<CsmController> {
        self.csm_controller.clone()
    }

    /// Returns a new broadcast `Receiver` handle to the consensus update
    /// notification queue.  Provides no guarantees about which position in the
    /// queue will be returned on the first receive.
    pub fn create_cstate_subscription(&self) -> broadcast::Receiver<Arc<ClientUpdateNotif>> {
        self.cupdate_rx.resubscribe()
    }

    pub fn status_rx(&self) -> Arc<StatusRx> {
        self.status_rx.clone()
    }

    pub fn status_tx(&self) -> Arc<StatusTx> {
        self.status_tx.clone()
    }

    /// Submits a fork choice message if possible. (synchronously)
    pub fn submit_chain_tip_msg(&self, ctm: ForkChoiceMessage) -> bool {
        self.fc_manager_tx.blocking_send(ctm).is_ok()
    }

    /// Submits a fork choice message if possible. (asynchronously)
    pub async fn submit_chain_tip_msg_async(&self, ctm: ForkChoiceMessage) -> bool {
        self.fc_manager_tx.send(ctm).await.is_ok()
    }
}

/// Starts the sync tasks using provided settings.
#[allow(clippy::too_many_arguments)]
pub fn start_sync_tasks<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    executor: &TaskExecutor,
    database: Arc<D>,
    l2_block_manager: Arc<L2BlockManager>,
    engine: Arc<E>,
    pool: threadpool::ThreadPool,
    params: Arc<Params>,
    status_bundle: (Arc<StatusTx>, Arc<StatusRx>),
    checkpoint_manager: Arc<CheckpointDbManager>,
) -> anyhow::Result<SyncManager> {
    // Create channels.
    let (fcm_tx, fcm_rx) = mpsc::channel::<ForkChoiceMessage>(64);
    let (csm_tx, csm_rx) = mpsc::channel::<CsmMessage>(64);
    let csm_controller = Arc::new(CsmController::new(database.clone(), pool, csm_tx));

    // TODO should this be in an `Arc`?  it's already fairly compact so we might
    // not be benefitting from the reduced cloning
    let (cupdate_tx, cupdate_rx) = broadcast::channel::<Arc<ClientUpdateNotif>>(64);

    // Start the fork choice manager thread.  If we haven't done genesis yet
    // this will just wait until the CSM says we have.
    let fcm_database = database.clone();
    let fcm_l2_block_manager = l2_block_manager.clone();
    let fcm_engine = engine.clone();
    let fcm_csm_controller = csm_controller.clone();
    let fcm_params = params.clone();
    let status_rx = status_bundle.1.clone();
    executor.spawn_critical("fork_choice_manager::tracker_task", |shutdown| {
        // TODO this should be simplified into a builder or something
        fork_choice_manager::tracker_task(
            shutdown,
            fcm_database,
            fcm_l2_block_manager,
            fcm_engine,
            fcm_rx,
            fcm_csm_controller,
            fcm_params,
            status_rx,
        )
    });

    // Prepare the client worker state and start the thread for that.
    let client_worker_state = worker::WorkerState::open(
        params.clone(),
        database,
        l2_block_manager,
        cupdate_tx,
        checkpoint_manager,
    )?;

    let csm_engine = engine.clone();

    let status_tx = status_bundle.0.clone();
    executor.spawn_critical("client_worker_task", |shutdown| {
        worker::client_worker_task(shutdown, client_worker_state, csm_engine, csm_rx, status_tx)
            .map_err(Into::into)
    });

    Ok(SyncManager {
        params,
        fc_manager_tx: fcm_tx,
        csm_controller,
        cupdate_rx,
        status_tx: status_bundle.0,
        status_rx: status_bundle.1,
    })
}
