//! Manages and updates unified status bundles
use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use strata_rpc_types::L1Status;
use strata_state::{chain_state::ChainState, client_state::ClientState, csm_status::CsmStatus};
use thiserror::Error;
use tokio::sync::watch;
use tracing::warn;

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitialized,

    #[error("{0}")]
    Other(String),
}

const GENESIS_CHECK_INTERVAL: u64 = 10; // Millis

/// Bundle wrapper for Status receiver
pub struct StatusRx {
    csm: watch::Receiver<CsmStatus>,
    cl: watch::Receiver<ClientState>,
    l1: watch::Receiver<L1Status>,
    chs: watch::Receiver<Option<ChainState>>,
    condvar_pair: Arc<(Mutex<bool>, Condvar)>,
}

impl StatusRx {
    pub fn new(
        csm: watch::Receiver<CsmStatus>,
        cl: watch::Receiver<ClientState>,
        l1: watch::Receiver<L1Status>,
        chs: watch::Receiver<Option<ChainState>>,
    ) -> Self {
        let condvar_pair = Arc::new((Mutex::new(true), Condvar::new()));
        let pair_clone = condvar_pair.clone();
        let cl_clone = cl.clone();

        let status_rx = Self {
            csm,
            cl,
            l1,
            chs,
            condvar_pair,
        };

        // Spawn a thread that waits for genesis and notifies condvar. This will be used for
        // waiting until genesis by calling `wait_until_genesis` method below.
        thread::spawn(move || loop {
            let cstate = cl_clone.borrow();
            if cstate.has_genesis_occured() {
                let (lock, cvar) = &*pair_clone;
                let mut pending = lock.lock().unwrap();
                *pending = false;
                cvar.notify_one();
                break;
            }
            thread::sleep(Duration::from_millis(GENESIS_CHECK_INTERVAL));
        });

        status_rx
    }

    pub fn csm(&self) -> &watch::Receiver<CsmStatus> {
        &self.csm
    }

    pub fn cl(&self) -> &watch::Receiver<ClientState> {
        &self.cl
    }

    pub fn l1(&self) -> &watch::Receiver<L1Status> {
        &self.l1
    }

    pub fn chs(&self) -> &watch::Receiver<Option<ChainState>> {
        &self.chs
    }

    pub fn wait_until_genesis(&self) -> ClientState {
        let cstate = self.cl.borrow().clone();
        if cstate.has_genesis_occured() {
            return cstate;
        }

        let (lock, cvar) = &*self.condvar_pair;
        let _guard = cvar
            .wait_while(lock.lock().unwrap(), |pending| *pending)
            .unwrap();

        self.cl.borrow().clone()
    }
}

impl StatusTx {
    pub fn update_chain_state(&self, post_state: &ChainState) {
        if self.chs.send(Some(post_state.clone())).is_err() {
            warn!("chain state receiver dropped");
        }
    }

    pub fn update_client_state(&self, post_state: &ClientState) {
        if self.cl.send(post_state.clone()).is_err() {
            warn!("client state receiver dropped");
        }
    }

    pub fn update_l1_status(&self, post_state: &L1Status) {
        if self.l1.send(post_state.clone()).is_err() {
            warn!("l1 status receiver dropped");
        }
    }

    pub fn update_csm_status(&self, post_state: &CsmStatus) {
        if self.csm.send(post_state.clone()).is_err() {
            warn!("csm status receiver dropped");
        }
    }
}

/// Bundle wrapper for Status sender
pub struct StatusTx {
    pub csm: watch::Sender<CsmStatus>,
    pub cl: watch::Sender<ClientState>,
    pub l1: watch::Sender<L1Status>,
    pub chs: watch::Sender<Option<ChainState>>,
}

/// initializes the StatusRx and StatusTx watch channel wrapper
pub fn create_status_channel(
    csm: CsmStatus,
    cl: ClientState,
    l1: L1Status,
) -> (Arc<StatusTx>, Arc<StatusRx>) {
    let (csm_tx, csm_rx) = watch::channel(csm);
    let (cl_tx, cl_rx) = watch::channel(cl);
    let (l1_tx, l1_rx) = watch::channel(l1);
    let (chs_tx, chs_rx) = watch::channel(None);

    (
        Arc::new(StatusTx {
            csm: csm_tx,
            cl: cl_tx,
            l1: l1_tx,
            chs: chs_tx,
        }),
        Arc::new(StatusRx::new(csm_rx, cl_rx, l1_rx, chs_rx)),
    )
}
