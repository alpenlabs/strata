//! Manages and updates unified status bundles
use std::sync::Arc;

use alpen_express_rpc_types::L1Status;
use alpen_express_state::{
    chain_state::ChainState, client_state::ClientState, csm_status::CsmStatus,
};
use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitialized,

    #[error("{0}")]
    Other(String),
}

/// Bundle wrapper for Status receiver
pub struct StatusRx {
    pub csm: watch::Receiver<CsmStatus>,
    pub cl: watch::Receiver<ClientState>,
    pub l1: watch::Receiver<L1Status>,
    pub chs: watch::Receiver<Option<ChainState>>,
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
        Arc::new(StatusRx {
            csm: csm_rx,
            cl: cl_rx,
            l1: l1_rx,
            chs: chs_rx,
        }),
    )
}
