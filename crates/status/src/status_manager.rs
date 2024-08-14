//! Manages and updates unified status bundles
use std::sync::Arc;

use thiserror::Error;
use tokio::sync::watch;
use tracing::error;

use alpen_express_rpc_types::L1Status;
use alpen_express_state::{client_state::ClientState, csm_status::CsmStatus};

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitialized,

    #[error("{0}")]
    Other(String),
}

/// Bundles the individual status into single collection
#[derive(Debug, Clone, Default)]
pub struct StatusBundle {
    pub csm: Option<CsmStatus>,
    pub cl: Option<ClientState>,
    pub l1: Option<L1Status>,
}

/// For updating individual status in StatusBundle
pub enum UpdateStatus {
    UpdateL1(L1Status),
    UpdateCl(ClientState),
    UpdateCsm(CsmStatus),
}

/// wrapper for StatusBundle receiver
pub struct StatusRx {
    rx: watch::Receiver<StatusBundle>,
}

impl StatusRx {
    /// Retrieves the last `StatusBundle` seen by Rx
    pub fn get(&self) -> StatusBundle {
        self.rx.borrow().clone()
    }
}

/// wrapper for StatusBundle sender
pub struct StatusTx {
    tx: watch::Sender<StatusBundle>,
}

impl StatusTx {
    /// Retrieves the last `StatusBundle` seen by Tx
    pub fn get_recent(&self) -> StatusBundle {
        self.tx.borrow().clone()
    }

    /// Retrieves most recent `StatusBundle` and applies the update based on `UpdateStatus`
    pub fn update_status(&self, update_status: &[UpdateStatus]) -> Result<(), StatusError> {
        let bundle = self.get_recent();
        let mut new_bundle = StatusBundle {
            csm: bundle.csm.clone(),
            cl: bundle.cl.clone(),
            l1: bundle.l1.clone(),
        };
        drop(bundle);

        for update in update_status {
            match update {
                UpdateStatus::UpdateL1(l1_status) => {
                    new_bundle.l1 = Some(l1_status.clone());
                }
                UpdateStatus::UpdateCl(client_state) => {
                    new_bundle.cl = Some(client_state.clone());
                }
                UpdateStatus::UpdateCsm(csm_status) => {
                    new_bundle.csm = Some(csm_status.clone());
                }
            };
        }

        if self.tx.send(new_bundle).is_err() {
            return Err(StatusError::NotInitialized);
        }

        Ok(())
    }
}

/// initializes the StatusRx and StatusTx watch channel wrapper
pub fn create_status_channel() -> (Arc<StatusRx>, Arc<StatusTx>) {
    let (st_tx, st_rx) = watch::channel(StatusBundle::default());
    (
        Arc::new(StatusRx { rx: st_rx }),
        Arc::new(StatusTx { tx: st_tx }),
    )
}
