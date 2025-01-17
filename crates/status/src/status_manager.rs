//! Manages and updates unified status bundles
use std::{cell::Ref, sync::Arc};

use strata_primitives::{l1::L1Status, l2::L2BlockId};
use strata_state::{
    bridge_state::{DepositsTable, OperatorTable},
    chain_state::Chainstate,
    client_state::{ClientState, L1Checkpoint, LocalL1State, SyncState},
    fcm_state::FcmState,
};
use thiserror::Error;
use tokio::sync::{
    broadcast,
    watch::{self, error::RecvError},
};
use tracing::warn;

pub const CHAINSTATE_BUFFER_CNT: usize = 256;

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
#[derive(Clone)]
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
        let (chs_tx, chs_rx) = watch::channel(ch_state.clone());
        let (chs_seq_tx, chs_seq_rx) = broadcast::channel(CHAINSTATE_BUFFER_CNT);

        let sender = Arc::new(StatusSender {
            csm_tx: cl_tx,
            l1_tx,
            chs_tx,
            fcm_tx: chs_seq_tx,
        });
        let receiver = Arc::new(StatusReceiver {
            csm_rx: cl_rx,
            l1_rx,
            chs_rx,
            fcm_rx: chs_seq_rx,
        });

        Self { sender, receiver }
    }

    // Receiver methods

    /// Gets the latest [`LocalL1State`].
    pub fn get_l1_view(&self) -> LocalL1State {
        self.receiver.csm_rx.borrow().l1_view().clone()
    }

    /// Gets the last finalized [`L1Checkpoint`] from the current client state.
    pub fn get_last_checkpoint(&self) -> Option<L1Checkpoint> {
        self.receiver
            .csm_rx
            .borrow()
            .l1_view()
            .last_finalized_checkpoint()
            .cloned()
    }

    /// Gets the latest [`SyncState`].
    pub fn get_sync_state(&self) -> Option<SyncState> {
        self.receiver.csm_rx.borrow().sync().cloned()
    }

    fn get_chainstate_cloned(&self) -> Option<Chainstate> {
        self.receiver.chs_rx.borrow().clone()
    }

    /// Gets the current L2 chain tip, if set.
    ///
    /// Returns a tuple of (epoch, slot, blkid).
    pub fn get_cur_l2_tip(&self) -> Option<(u64, u64, L2BlockId)> {
        let chs = self.get_chainstate_cloned();
        chs.map(|chs| {
            (
                chs.cur_epoch(),
                chs.chain_tip_slot(),
                chs.chain_tip_blockid(),
            )
        })
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
        self.receiver.l1_rx.borrow().clone()
    }

    /// Gets the latest epoch
    pub fn epoch(&self) -> Option<u64> {
        self.receiver
            .chs_rx
            .borrow()
            .to_owned()
            .map(|ch| ch.cur_epoch())
    }

    pub fn chain_state(&self) -> Option<Chainstate> {
        self.receiver.chs_rx.borrow().clone()
    }

    pub fn client_state(&self) -> ClientState {
        self.receiver.csm_rx.borrow().clone()
    }

    /// Creates a subscription to the client state channel.
    pub fn subscribe_client_state(&self) -> watch::Receiver<ClientState> {
        self.sender.csm_tx.subscribe()
    }

    /// Creates a subscription to the chainstate channel.
    pub fn subscribe_fcm_state(&self) -> broadcast::Receiver<FcmState> {
        self.sender.fcm_tx.subscribe()
    }

    /// Waits until there's a new client state and returns the client state.
    pub async fn wait_for_client_change(&self) -> Result<ClientState, RecvError> {
        let mut s = self.receiver.csm_rx.clone();
        s.mark_unchanged();
        s.changed().await?;
        let state = s.borrow_and_update().clone();
        Ok(state)
    }

    /// Waits until genesis and returns the client state.
    pub async fn wait_until_genesis(&self) -> Result<ClientState, RecvError> {
        let mut rx = self.receiver.csm_rx.clone();
        let chs = rx.wait_for(|chs| chs.has_genesis_occurred()).await?;
        Ok(chs.clone())
    }

    // Sender methods

    /// Sends the updated `Chainstate` to the chain state receiver. Logs a warning if the receiver
    /// is dropped.
    pub fn update_chainstate(&self, post_state: Arc<Chainstate>) {
        if self
            .sender
            .chs_tx
            .send(Some(post_state.as_ref().clone()))
            .is_err()
        {
            warn!("chain state receiver dropped");
        }
    }

    pub fn update_fcm_state(&self, state: FcmState) {
        let _ = self.sender.fcm_tx.send(state);
    }

    /// Sends the updated `ClientState` to the client state receiver. Logs a warning if the receiver
    /// is dropped.
    pub fn update_client_state(&self, post_state: ClientState) {
        if self.sender.csm_tx.send(post_state).is_err() {
            warn!("client state receiver dropped");
        }
    }

    /// Sends the updated `L1Status` to the L1 status receiver. Logs a warning if the receiver is
    /// dropped.
    pub fn update_l1_status(&self, post_state: L1Status) {
        if self.sender.l1_tx.send(post_state).is_err() {
            warn!("l1 status receiver dropped");
        }
    }
}

/// Wrapper for watch status receivers.
struct StatusReceiver {
    csm_rx: watch::Receiver<ClientState>,
    l1_rx: watch::Receiver<L1Status>,
    chs_rx: watch::Receiver<Option<Chainstate>>,
    fcm_rx: broadcast::Receiver<FcmState>,
}

/// Wrapper for watch status senders.
struct StatusSender {
    csm_tx: watch::Sender<ClientState>,
    l1_tx: watch::Sender<L1Status>,
    chs_tx: watch::Sender<Option<Chainstate>>,
    fcm_tx: broadcast::Sender<FcmState>,
}
