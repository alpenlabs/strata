#![allow(dead_code)]

use std::sync::Arc;

use thiserror::Error;
use tokio::sync::watch::{Receiver, Sender};
use tokio::sync::{watch, RwLock};
use tracing::error;

use alpen_express_state::{client_state::ClientState, csm_status::CsmStatus};

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitializedError,

    #[error("{0}")]
    Other(String),
}

#[derive(Clone, Default)]
pub struct Watch<T> {
    rx: Option<Receiver<T>>,
    tx: Option<Sender<T>>,
}

impl<T: Clone> Watch<T> {
    pub fn new() -> Self {
        Self { rx: None, tx: None }
    }
    // Create a new Wrapper
    pub fn init(&mut self, val: T) {
        let (tx, rx) = watch::channel(val);
        self.tx = Some(tx);
        self.rx = Some(rx);
    }

    // borrow rx
    pub fn borrow_rx(&self) -> Result<Receiver<T>, StatusError> {
        match &self.rx {
            Some(rx) => Ok(rx.clone()),
            None => Err(StatusError::NotInitializedError),
        }
    }

    // Method to get the current state
    pub fn get(&self) -> Result<T, StatusError> {
        match &self.rx {
            Some(rx) => Ok(rx.borrow().clone()),
            None => Err(StatusError::NotInitializedError),
        }
    }

    // Method to update the state
    pub fn send(&self, new_value: T) -> Result<(), StatusError> {
        match &self.tx {
            Some(tx) => {
                if tx.send(new_value).is_err() {
                    error!("failed to submit new CSM status update");
                }
                Ok(())
            }
            None => Err(StatusError::NotInitializedError),
        }
    }
}

pub struct NodeStatus {
    l1_status: RwLock<L1Status>,
    csm_status: RwLock<Watch<CsmStatus>>,
    cl_state: RwLock<Watch<Arc<ClientState>>>,
}

impl NodeStatus {
    pub async fn update_l1_status(&self, l1_status: &L1Status) {
        let mut l1_status_writer = self.l1_status.write().await;
        *l1_status_writer = l1_status.clone();
    }

    pub fn update_csm_status(&self, csm_status: &CsmStatus) {
        let mut csm_writer = self.csm_status.blocking_write();
        csm_writer.init(csm_status.clone());
    }

    pub fn update_cl_state(&self, cl_state: Arc<ClientState>) {
        let mut cl_writer = self.cl_state.blocking_write();
        cl_writer.init(cl_state)
    }

    pub fn csm_status_watch(&self) -> Watch<CsmStatus> {
        self.csm_status.blocking_read().clone()
    }

    pub fn cl_state_watch(&self) -> Watch<Arc<ClientState>> {
        self.cl_state.blocking_read().clone()
    }

    pub async fn l1_status(&self) -> L1Status {
        self.l1_status.read().await.clone()
    }
}

impl Default for NodeStatus {
    fn default() -> Self {
        Self {
            l1_status: RwLock::new(L1Status::default()),
            csm_status: RwLock::new(Watch::new()),
            cl_state: RwLock::new(Watch::new()),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum Status {
    L1Status,
    CSMStatus,
    ClientState,
}

pub struct NodeStatus2 {
    readers: Vec<Arc<dyn StatusReader>>,
    writers: Vec<Arc<dyn StatusWriter>>,
}

impl NodeStatus2 {
    pub fn new() -> Self {
        Self {
            readers: Vec::new(),
            writers: Vec::new(),
        }
    }

    pub fn add_reader(&mut self, reader: Arc<dyn StatusReader>) {
        self.readers.push(reader);
    }

    pub fn add_writer(&mut self, writer: Arc<dyn StatusWriter>) {
        self.writers.push(writer);
    }

    pub fn get_reader(&self, status: Status) -> Option<Arc<dyn StatusReader>> {
        for reader in self.readers.clone() {
            if reader.name() == status {
                return Some(reader.clone());
            }
        }
        None
    }

    pub fn get_writer(&mut self, status: Status) -> Option<Arc<dyn StatusWriter>> {
        for writer in self.writers.clone() {
            if writer.name() == status {
                return Some(writer.clone());
            }
        }
        None
    }
}

pub trait StatusStruct {
    fn name(&self) -> Status;
}

pub trait StatusReader {
    fn read(&self) -> Arc<dyn StatusStruct>;
    fn name(&self) -> Status;
}

pub trait StatusWriter {
    fn write(&self, new_val: Arc<dyn StatusStruct>);
    fn name(&self) -> Status;
}

impl StatusReader for watch::Receiver<Arc<dyn StatusStruct>> {
    fn read(&self) -> Arc<dyn StatusStruct> {
        self.borrow().clone()
    }

    fn name(&self) -> Status {
        self.borrow().name()
    }
}

impl StatusWriter for watch::Sender<Arc<dyn StatusStruct>> {
    fn write(&self, new_value: Arc<dyn StatusStruct>) {
        if self.send(new_value).is_err() {
            error!("failed to submit new status update for");
        }
    }

    fn name(&self) -> Status {
        self.borrow().name()
    }
}

impl StatusStruct for L1Status {
    fn name(&self) -> Status {
        Status::L1Status
    }
}
impl StatusStruct for CsmStatus {
    fn name(&self) -> Status {
        Status::CSMStatus
    }
}
impl StatusStruct for ClientState {
    fn name(&self) -> Status {
        Status::ClientState
    }
}
