//! Manages and updates unified status bundles
use std::{cell::Ref, sync::Arc};

use strata_primitives::l1::L1Status;
use strata_state::{
    bridge_state::{DepositsTable, OperatorTable},
    chain_state::Chainstate,
    client_state::{ClientState, L1Checkpoint, LocalL1State, SyncState},
};
use thiserror::Error;
use tokio::sync::watch::{self, error::RecvError};
use tracing::{instrument::WithSubscriber, warn};

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
    pub fn get_l1_view(&self) -> LocalL1State {
        self.receiver.cl.borrow().l1_view().clone()
    }

    /// Gets the last finalized [`L1Checkpoint`] from the current client state.
    pub fn get_last_checkpoint(&self) -> Option<L1Checkpoint> {
        self.receiver
            .cl
            .borrow()
            .l1_view()
            .last_finalized_checkpoint()
            .cloned()
    }

    /// Gets the latest [`SyncState`].
    pub fn get_sync_state(&self) -> Option<SyncState> {
        self.receiver.cl.borrow().sync().cloned()
    }

    fn get_chainstate_cloned(&self) -> Option<Chainstate> {
        self.receiver.chs.borrow().clone()
    }

    /// Gets the epoch of the current chain tip.
    pub fn get_cur_l2_epoch(&self) -> Option<u64> {
        self.get_chainstate_cloned().map(|chs| chs.cur_epoch())
    }

    /// Gets the latest operator table.
    pub fn get_cur_operator_table(&self) -> Option<OperatorTable> {
        self.get_chainstate_cloned()
            .map(|chs| chs.operator_table().clone())
    }

    /// Gets the latest deposits table.
    pub fn get_cur_deposits_table(&self) -> Option<DepositsTable> {
        self.get_chainstate_cloned()
            .map(|chs| chs.deposits_table().clone())
    }

    /// Gets the latest [`L1Status`].
    pub fn get_l1_reader_status(&self) -> L1Status {
        self.receiver.l1.borrow().clone()
    }

    /// Gets the latest epoch
    pub fn epoch(&self) -> Option<u64> {
        self.receiver
            .chs
            .borrow()
            .to_owned()
            .map(|ch| ch.cur_epoch())
    }

    pub fn chain_state(&self) -> Option<Chainstate> {
        self.receiver.chs.borrow().clone()
    }

    pub fn client_state(&self) -> ClientState {
        self.receiver.cl.borrow().clone()
    }

    /// Create a subscription to the client state watcher.
    pub fn subscribe_client_state(&self) -> watch::Receiver<ClientState> {
        self.sender.cl.subscribe()
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
