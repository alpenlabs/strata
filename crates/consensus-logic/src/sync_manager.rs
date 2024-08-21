//! High level sync manager which controls core sync tasks and manages sync
//! status.  Exposes handles to interact with fork choice manager and CSM
//! executor and other core sync pipeline tasks.

use std::sync::Arc;
use std::thread;

use alpen_express_state::client_state::ClientState;
use tokio::sync::{broadcast, mpsc, watch};
use tracing::*;

use alpen_express_db::traits::Database;
use alpen_express_eectl::engine::ExecEngineCtl;
use alpen_express_primitives::params::Params;
use express_storage::L2BlockManager;

use crate::ctl::CsmController;
use crate::message::{ClientUpdateNotif, CsmMessage, ForkChoiceMessage};
use crate::status::CsmStatus;
use crate::{fork_choice_manager, genesis, worker};

/// Handle to the core pipeline tasks.
pub struct SyncManager {
    params: Arc<Params>,

    fc_manager_tx: mpsc::Sender<ForkChoiceMessage>,
    csm_ctl: Arc<CsmController>,

    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
    cl_state_rx: watch::Receiver<Arc<ClientState>>,
    csm_status_rx: watch::Receiver<CsmStatus>,
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
        &self.csm_ctl
    }

    /// Gets a clone of the CSM controller.
    pub fn get_csm_ctl(&self) -> Arc<CsmController> {
        self.csm_ctl.clone()
    }

    /// Returns a new broadcast `Receiver` handle to the consensus update
    /// notification queue.  Provides no guarantees about which position in the
    /// queue will be returned on the first receive.
    pub fn create_cstate_subscription(&self) -> broadcast::Receiver<Arc<ClientUpdateNotif>> {
        self.cupdate_rx.resubscribe()
    }

    /// Returns a new watch `Receiver` handle to the CSM state watch.
    pub fn create_state_watch_sub(&self) -> watch::Receiver<Arc<ClientState>> {
        self.cl_state_rx.clone()
    }

    /// Gets a clone of the last sent CSM status.
    pub fn get_csm_status(&self) -> CsmStatus {
        self.csm_status_rx.borrow().clone()
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
pub fn start_sync_tasks<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    database: Arc<D>,
    l2_block_manager: Arc<L2BlockManager>,
    engine: Arc<E>,
    pool: threadpool::ThreadPool,
    params: Arc<Params>,
) -> anyhow::Result<SyncManager> {
    // Create channels.
    let (fcm_tx, fcm_rx) = mpsc::channel::<ForkChoiceMessage>(64);
    let (csm_tx, csm_rx) = mpsc::channel::<CsmMessage>(64);
    let csm_ctl = Arc::new(CsmController::new(database.clone(), pool, csm_tx));

    // TODO should this be in an `Arc`?  it's already fairly compact so we might
    // not be benefitting from the reduced cloning
    let (cupdate_tx, cupdate_rx) = broadcast::channel::<Arc<ClientUpdateNotif>>(64);

    // Check if we have to do genesis.
    if genesis::check_needs_client_init(database.as_ref())? {
        info!("need to init client state!");
        genesis::init_client_state(&params, database.as_ref())?;
    }

    // Start the fork choice manager thread.  If we haven't done genesis yet
    // this will just wait until the CSM says we have.
    let fcm_db = database.clone();
    let fcm_l2blkman = l2_block_manager.clone();
    let fcm_eng = engine.clone();
    let fcm_csm_ctl = csm_ctl.clone();
    let fcm_params = params.clone();
    let _ct_handle = thread::spawn(|| {
        // TODO this should be simplified into a builder or something
        fork_choice_manager::tracker_task(
            fcm_db,
            fcm_l2blkman,
            fcm_eng,
            fcm_rx,
            fcm_csm_ctl,
            fcm_params,
        )
    });

    // Prepare the client worker state and start the thread for that.
    let cw_state = worker::WorkerState::open(
        params.clone(),
        database.clone(),
        l2_block_manager,
        cupdate_tx,
    )?;
    let state = cw_state.cur_state().clone();

    let mut status = CsmStatus::default();
    status.set_last_sync_ev_idx(cw_state.cur_event_idx());
    status.update_from_client_state(state.as_ref());
    let (csm_status_tx, csm_status_rx) = watch::channel(status);
    let (cl_state_tx, cl_state_rx) = watch::channel(state);

    let csm_eng = engine.clone();
    let csm_fcm_tx = fcm_tx.clone();
    let _cw_handle = thread::spawn(|| {
        worker::client_worker_task(
            cw_state,
            csm_eng,
            csm_rx,
            cl_state_tx,
            csm_status_tx,
            csm_fcm_tx,
        )
    });

    Ok(SyncManager {
        params,
        fc_manager_tx: fcm_tx,
        csm_ctl,
        cupdate_rx,
        cl_state_rx,
        csm_status_rx,
    })
}
