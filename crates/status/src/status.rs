#![allow(dead_code)]

use std::sync::Arc;

use alpen_express_consensus_logic::status::CsmStatus;
use alpen_express_primitives::l1::L1Status;
use alpen_express_state::client_state::ClientState;
use thiserror::Error;
use tokio::sync::{watch, RwLock};
use tokio::sync::watch::{Receiver, Sender};
use tracing::error;

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitializedError,

    #[error("{0}")]
    Other(String),
}

#[derive(Clone)]
pub struct Watch<T> {
    rx: Option<Receiver<T>>,
    tx: Option<Sender<T>>,
}

impl<T: Clone> Watch<T> {
    pub fn new() -> Self {
        Self { rx: None, tx: None }
    }
    // Create a new Wrapper
    pub fn init(&mut self,val: T) {
        let (tx, rx) = watch::channel(val);
        self.tx = Some(tx);
        self.rx = Some(rx);
    }

    // Method to get the current state
    pub fn get(&self) -> Result<T, StatusError> {
        if self.rx.is_some() {
            return Ok(self.rx.as_ref().unwrap().borrow().clone());
        }
        Err(StatusError::NotInitializedError)
    }

    // Method to update the state
    pub fn update(&self, new_value: T) -> Result<(), StatusError>{

        if self.tx.is_some() {
            if self.tx.as_ref().unwrap().send(new_value).is_err() {
                error!("failed to submit new CSM status update");
            }
            return Ok(())
        }
        Err(StatusError::NotInitializedError)
    }
}

#[derive(Clone)]
pub struct NodeStatus {
    pub l1_status: Arc<RwLock<L1Status>>,
    pub csm_status: Watch<CsmStatus>,
    pub cl_state: Watch<ClientState>
}



