//! High level sync manager which controls core sync tasks and manages sync
//! status.  Exposes handles to interact with fork choice manager and CSM
//! executor and other core sync pipeline tasks.

use std::sync::Arc;
use std::thread;

use alpen_vertex_state::client_state::ClientState;
use tokio::sync::{broadcast, mpsc, watch};
use tracing::*;

use alpen_vertex_db::traits::{Database, L2DataProvider};
use alpen_vertex_evmctl::engine::ExecEngineCtl;
use alpen_vertex_primitives::params::Params;

use crate::ctl::CsmController;
use crate::message::{ClientUpdateNotif, CsmMessage, ForkChoiceMessage};
use crate::{errors, fork_choice_manager, genesis, unfinalized_tracker, worker};

pub struct SyncManager {
    params: Arc<Params>,

    chain_tip_msg_tx: mpsc::Sender<ForkChoiceMessage>,
    csm_ctl: Arc<CsmController>,
    cupdate_rx: broadcast::Receiver<Arc<ClientUpdateNotif>>,
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

    /// Submits a fork choice message if possible. (synchronously)
    pub fn submit_chain_tip_msg(&self, ctm: ForkChoiceMessage) -> bool {
        self.chain_tip_msg_tx.blocking_send(ctm).is_ok()
    }

    /// Submits a fork choice message if possible. (asynchronously)
    pub async fn submit_chain_tip_msg_async(&self, ctm: ForkChoiceMessage) -> bool {
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
    pool: Arc<threadpool::ThreadPool>,
    params: Arc<Params>,
    cl_state_tx: watch::Sender<Option<ClientState>>,
) -> anyhow::Result<SyncManager> {
    // Create channels.
    let (ctm_tx, ctm_rx) = mpsc::channel::<ForkChoiceMessage>(64);
    let (csm_tx, csm_rx) = mpsc::channel::<CsmMessage>(64);
    let csm_ctl = Arc::new(CsmController::new(database.clone(), pool, csm_tx));

    // TODO should this be in an `Arc`?  it's already fairly compact so we might
    // not be benefitting from the reduced cloning
    let (cupdate_tx, cupdate_rx) = broadcast::channel::<Arc<ClientUpdateNotif>>(64);

    // Check if we have to do genesis.
    if genesis::check_needs_genesis(database.as_ref())? {
        info!("we need to do genesis!");
        genesis::init_genesis_states(&params, database.as_ref())?;
    }

    // Init the consensus worker state and get the current state from it.
    let cw_state = worker::WorkerState::open(params.clone(), database.clone(), cupdate_tx)?;
    let cur_state = cw_state.cur_state().clone();
    let cur_tip_blkid = *cur_state.chain_tip_blkid();
    let fin_tip_blkid = *cur_state.finalized_blkid();

    // Get the block's index.
    let l2_prov = database.l2_provider();
    let tip_block = l2_prov
        .get_block_data(cur_tip_blkid)?
        .ok_or(errors::Error::MissingL2Block(cur_tip_blkid))?;
    let cur_tip_index = tip_block.header().blockidx();

    let fin_block = l2_prov
        .get_block_data(fin_tip_blkid)?
        .ok_or(errors::Error::MissingL2Block(fin_tip_blkid))?;
    let fin_tip_index = fin_block.header().blockidx();

    // Init the chain tracker from the state we figured out.
    let mut chain_tracker = unfinalized_tracker::UnfinalizedBlockTracker::new_empty(fin_tip_blkid);
    chain_tracker.load_unfinalized_blocks(fin_tip_index + 1, l2_prov.as_ref())?;
    let ct_state = fork_choice_manager::ForkChoiceManager::new(
        params.clone(),
        database.clone(),
        cur_state,
        chain_tracker,
        cur_tip_blkid,
        cur_tip_index,
    );

    // Start core threads.
    // TODO set up watchdog for these things
    let eng_ct = engine.clone();
    let eng_cw = engine.clone();
    let ctl_ct = csm_ctl.clone();
    let _ct_handle =
        thread::spawn(|| fork_choice_manager::tracker_task(ct_state, eng_ct, ctm_rx, ctl_ct));
    let _cw_handle =
        thread::spawn(|| worker::consensus_worker_task(cw_state, eng_cw, csm_rx, cl_state_tx));

    // TODO do something with the handles

    Ok(SyncManager {
        params,
        chain_tip_msg_tx: ctm_tx,
        csm_ctl,
        cupdate_rx,
    })
}
