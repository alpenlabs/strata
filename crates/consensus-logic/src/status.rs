//! Handle to inspect the current consensus state and wait for updates when there are any.

use tokio::sync::watch;

pub struct StatusTracker {
    state_rx: watch::Receiver<()>,
}

pub struct StatusUpdater {
    state_tx: watch::Sender<()>,
}
