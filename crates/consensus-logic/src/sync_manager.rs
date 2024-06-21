//! High level sync manager which controls core sync tasks and manages sync
//! status.  Exposes handles to interact with chain tip tracker and CSM
//! executor and other core sync pipeline tasks.

use std::sync::Arc;
use std::thread;

use tokio::sync::{broadcast, mpsc};
use tracing::*;

use alpen_vertex_db::traits::Database;
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::params::Params;

use crate::ctl::CsmController;
use crate::message::{ChainTipMessage, ConsensusUpdateNotif, CsmMessage};
use crate::{chain_tip, genesis, unfinalized_tracker, worker};

pub struct SyncManager<D: Database> {
    params: Arc<Params>,

    chain_tip_msg_tx: mpsc::Sender<ChainTipMessage>,
    csm_ctl: Arc<CsmController<D>>,
    cupdate_rx: broadcast::Receiver<Arc<ConsensusUpdateNotif>>,
}

impl<D: Database> SyncManager<D> {
    pub fn params(&self) -> &Params {
        &self.params
    }

    pub fn get_params(&self) -> Arc<Params> {
        self.params.clone()
    }

    pub fn database(&self) -> &Arc<D> {
        self.csm_ctl.database()
    }

    pub fn csm_controller(&self) -> &CsmController<D> {
        &self.csm_ctl
    }

    /// Returns a new broadcast `Receiver` handle to the consensus update
    /// notification queue.  Provides no guarantees about which position in the
    /// queue will be returned on the first receive.
    pub fn create_cstate_subscription(&self) -> broadcast::Receiver<Arc<ConsensusUpdateNotif>> {
        self.cupdate_rx.resubscribe()
    }

    /// Submits a chain tip message if possible. (synchronously)
    pub fn submit_chain_tip_msg(&self, ctm: ChainTipMessage) -> bool {
        self.chain_tip_msg_tx.blocking_send(ctm).is_ok()
    }

    /// Submits a chain tip message if possible. (asynchronously)
    pub async fn submit_chain_tip_msg_async(&self, ctm: ChainTipMessage) -> bool {
        self.chain_tip_msg_tx.send(ctm).await.is_ok()
    }
}

/// Starts the sync tasks using provided settings.
pub fn start_sync_tasks<
    D: Database + Sync + Send + 'static,
    E: ExecEngineCtl + Sync + Send + 'static,
>(
    database: Arc<D>,
    engine: Arc<E>,
    params: Arc<Params>,
) -> anyhow::Result<SyncManager<D>> {
    // Create channels.
    let (ctm_tx, ctm_rx) = mpsc::channel::<ChainTipMessage>(64);
    let (csm_tx, csm_rx) = mpsc::channel::<CsmMessage>(64);
    let csm_ctl = Arc::new(CsmController::new(database.clone(), csm_tx));
    let (cupdate_tx, cupdate_rx) = broadcast::channel::<Arc<ConsensusUpdateNotif>>(64);

    // Check if we have to do genesis.
    if genesis::check_needs_genesis(database.as_ref())? {
        info!("we need to do genesis!");
        genesis::init_genesis_states(&params, database.as_ref())?;
    }

    // Init the consensus worker state and get the current state from it.
    let cw_state = worker::WorkerState::open(params.clone(), database.clone(), cupdate_tx)?;
    let cur_state = cw_state.cur_state().clone();
    let cur_chain_tip = cur_state.chain_state().chain_tip_blockid();

    // Init the chain tracker from the state we figured out.
    let chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(cur_chain_tip);
    let ct_state = chain_tip::ChainTipTrackerState::new(
        params.clone(),
        database.clone(),
        cur_state,
        chain_tracker,
        cur_chain_tip,
    );
    // TODO load unfinalized blocks into block tracker

    // Start core threads.
    // TODO set up watchdog for these things
    let eng_ct = engine.clone();
    let eng_cw = engine.clone();
    let ctl_ct = csm_ctl.clone();
    let ct_handle = thread::spawn(|| chain_tip::tracker_task(ct_state, eng_ct, ctm_rx, ctl_ct));
    let cw_handle = thread::spawn(|| worker::consensus_worker_task(cw_state, eng_cw, csm_rx));

    // TODO do something with the handles

    Ok(SyncManager {
        params,
        chain_tip_msg_tx: ctm_tx,
        csm_ctl,
        cupdate_rx,
    })
}
