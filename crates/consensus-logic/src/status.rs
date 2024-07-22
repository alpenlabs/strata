//! Handle to inspect the current CSM state and wait for updates when there are
//! any.

use std::sync::Arc;

use tokio::sync::RwLock;

use alpen_vertex_state::client_state::ClientState;

pub struct StatusTracker {
    cur_state: Arc<RwLock<Arc<ClientState>>>,
}

pub struct StatusUpdater {
    cur_state: Arc<RwLock<Arc<ClientState>>>,
}

pub fn make_csm_pair(cur_state: Arc<ClientState>) -> (StatusTracker, StatusUpdater) {
    let cur_state = Arc::new(RwLock::new(cur_state));

    let tracker = StatusTracker {
        cur_state: cur_state.clone(),
    };

    let updater = StatusUpdater { cur_state };

    (tracker, updater)
}
