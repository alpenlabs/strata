//! Manages and updates unified status bundles
use std::sync::Arc;

use strata_rpc_types::L1Status;
use strata_state::{chain_state::Chainstate, client_state::ClientState, csm_status::CsmStatus};
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

/// Bundle wrapper for Status receiver
pub struct StatusRx {
    csm: watch::Receiver<CsmStatus>,
    cl: watch::Receiver<ClientState>,
    l1: watch::Receiver<L1Status>,
    chs: watch::Receiver<Option<Chainstate>>,
}

impl StatusRx {
    pub fn new(
        csm: watch::Receiver<CsmStatus>,
        cl: watch::Receiver<ClientState>,
        l1: watch::Receiver<L1Status>,
        chs: watch::Receiver<Option<Chainstate>>,
    ) -> Self {
        Self { csm, cl, l1, chs }
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

    pub fn chs(&self) -> &watch::Receiver<Option<Chainstate>> {
        &self.chs
    }
}

impl StatusTx {
    pub fn update_chain_state(&self, post_state: &Chainstate) {
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
    pub chs: watch::Sender<Option<Chainstate>>,
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
