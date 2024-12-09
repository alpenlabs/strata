//! Manages and updates unified status bundles
use std::sync::Arc;

use strata_primitives::l1::L1Status;
use strata_state::{
    bridge_state::{DepositsTable, OperatorTable},
    chain_state::Chainstate,
    client_state::{ClientState, LocalL1State, SyncState},
};
use thiserror::Error;
use tokio::sync::watch::{self, error::RecvError};
use tracing::warn;

#[derive(Debug, Error)]
pub enum StatusError {
    #[error("not initialized yet")]
    NotInitialized,

    #[error("{0}")]
    Other(String),
}

/// A wrapper around the status sender and receiver.
///
/// This struct provides a convenient way to manage and access
/// both the sender and receiver components of a status communication channel.
#[derive(Clone, Debug)]
pub struct StatusChannel {
    /// Shared reference to the status sender.
    sender: Arc<StatusSender>,
    /// Shared reference to the status receiver.
    receiver: Arc<StatusReceiver>,
}

impl StatusChannel {
    /// Creates a new `StatusChannel` for managing communication between components.
    ///
    /// # Arguments
    ///
    /// * `cl_state` - Initial state for the client.
    /// * `l1_status` - Initial L1 status.
    /// * `ch_state` - Optional initial chainstate.
    ///
    /// # Returns
    ///
    /// A `StatusChannel` containing a sender and receiver for the provided states.
    pub fn new(cl_state: ClientState, l1_status: L1Status, ch_state: Option<Chainstate>) -> Self {
        let (cl_tx, cl_rx) = watch::channel(cl_state);
        let (l1_tx, l1_rx) = watch::channel(l1_status);
        let (chs_tx, chs_rx) = watch::channel(ch_state);

        let sender = Arc::new(StatusSender {
            cl: cl_tx,
            l1: l1_tx,
            chs: chs_tx,
        });
        let receiver = Arc::new(StatusReceiver {
            cl: cl_rx,
            l1: l1_rx,
            chs: chs_rx,
        });

        Self { sender, receiver }
    }

    // Receiver methods

    /// Gets the latest [`LocalL1State`].
    pub fn l1_view(&self) -> LocalL1State {
        self.receiver.cl.borrow().l1_view().clone()
    }

    /// Gets the latest [`SyncState`].
    pub fn sync_state(&self) -> Option<SyncState> {
        self.receiver.cl.borrow().sync().cloned()
    }

    /// Gets the latest operator table.
    pub fn operator_table(&self) -> Option<OperatorTable> {
        self.receiver
            .chs
            .borrow()
            .clone()
            .map(|chs| chs.operator_table().clone())
    }

    /// Gets the latest deposits table.
    pub fn deposits_table(&self) -> Option<DepositsTable> {
        self.receiver
            .chs
            .borrow()
            .clone()
            .map(|chs| chs.deposits_table().clone())
    }

    /// Gets the latest [`L1Status`].
    pub fn l1_status(&self) -> L1Status {
        self.receiver.l1.borrow().clone()
    }

    pub fn epoch(&self) -> Option<u64> {
        self.receiver.chs.borrow().to_owned().map(|ch| ch.epoch())
    }

    /// Waits until there's a new client state and returns the client state.
    pub async fn wait_for_client_change(&self) -> Result<ClientState, RecvError> {
        let mut s = self.receiver.cl.clone();
        s.changed().await?;
        let state = s.borrow().clone();
        Ok(state)
    }

    /// Waits until genesis and returns the client state.
    pub async fn wait_until_genesis(&self) -> Result<ClientState, RecvError> {
        let mut rx = self.receiver.cl.clone();
        loop {
            if rx.borrow().has_genesis_occurred() {
                return Ok(rx.borrow().clone());
            }
            rx.changed().await?;
        }
    }

    // Sender methods

    /// Sends the updated `Chainstate` to the chain state receiver. Logs a warning if the receiver
    /// is dropped.
    pub fn update_chainstate(&self, post_state: Chainstate) {
        if self.sender.chs.send(Some(post_state)).is_err() {
            warn!("chain state receiver dropped");
        }
    }

    /// Sends the updated `ClientState` to the client state receiver. Logs a warning if the receiver
    /// is dropped.
    pub fn update_client_state(&self, post_state: ClientState) {
        if self.sender.cl.send(post_state).is_err() {
            warn!("client state receiver dropped");
        }
    }

    /// Sends the updated `L1Status` to the L1 status receiver. Logs a warning if the receiver is
    /// dropped.
    pub fn update_l1_status(&self, post_state: L1Status) {
        if self.sender.l1.send(post_state).is_err() {
            warn!("l1 status receiver dropped");
        }
    }
}

/// Wrapper for watch status receivers
#[derive(Clone, Debug)]
struct StatusReceiver {
    cl: watch::Receiver<ClientState>,
    l1: watch::Receiver<L1Status>,
    chs: watch::Receiver<Option<Chainstate>>,
}

/// Wrapper for watch status senders
#[derive(Clone, Debug)]
struct StatusSender {
    cl: watch::Sender<ClientState>,
    l1: watch::Sender<L1Status>,
    chs: watch::Sender<Option<Chainstate>>,
}
