#![allow(dead_code)]
use alpen_express_rpc_types::L1Status;
use thiserror::Error;
use tokio::sync::watch;
use tracing::error;

use alpen_express_state::{client_state::ClientState, csm_status::CsmStatus};

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitializedError,

    #[error("{0}")]
    Other(String),
}

#[derive(Debug, Clone, Default)]
pub struct StatusBundle {
    pub csm: Option<CsmStatus>,
    pub cl: Option<ClientState>,
    pub l1: Option<L1Status>,
}

pub enum UpdateStatus {
    UpdateL1(L1Status),
    UpdateCl(ClientState),
    UpdateCsm(CsmStatus),
}

pub struct NodeStatus {
    tx: watch::Sender<StatusBundle>,
    rx: watch::Receiver<StatusBundle>,
}

impl Default for NodeStatus {
    fn default() -> Self {
        let (st_tx, st_rx) = watch::channel(StatusBundle::default());
        Self {
            tx: st_tx,
            rx: st_rx,
        }
    }
}

impl NodeStatus {
    pub fn update_status(&self, update_status: &[UpdateStatus]) -> Result<(), StatusError> {
        let bundle = self.rx.borrow();
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

        //TODO: custom error type for this
        let x = self.tx.send(new_bundle);
        println!("{:?}", x);
        println!("was sent successfully");

        Ok(())
    }

    pub fn get(&self) -> StatusBundle {
        self.rx.borrow().clone()
    }
}
