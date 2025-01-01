use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

/// Represents the context in which the system may need to "bail out."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BailContext {
    /// Bail out after a sync event.
    SyncEvent,
    /// Bail out when a new block is processed in FCM.
    FcmNewBlock,
    /// Bail out during block signing.
    SignBlock,
}

/// Singleton manager for `watch::Sender` and `watch::Receiver` used to communicate bail-out
/// contexts.
static BAIL_MANAGER: LazyLock<(
    watch::Sender<Option<BailContext>>,
    watch::Receiver<Option<BailContext>>,
)> = LazyLock::new(|| {
    let (sender, receiver) = watch::channel(None);

    (sender, receiver)
});

/// Publicly accessible `watch::Sender` for broadcasting bail-out context updates.
pub static BAIL_SENDER: LazyLock<watch::Sender<Option<BailContext>>> =
    LazyLock::new(|| BAIL_MANAGER.0.clone());

/// Publicly accessible `watch::Receiver` for subscribing to bail-out context updates.
pub static BAIL_RECEIVER: LazyLock<watch::Receiver<Option<BailContext>>> =
    LazyLock::new(|| BAIL_MANAGER.1.clone());

#[macro_export]
macro_rules! handle_bail_context {
    ($ctx_to_match:expr, $exit_code:expr) => {{
        let recv = *BAIL_RECEIVER.borrow();
        if let Some(ctx) = recv {
            if ctx == $ctx_to_match {
                std::process::exit($exit_code);
            }
        }
    }};
}
