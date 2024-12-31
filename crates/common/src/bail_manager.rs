use std::sync::LazyLock;

use tokio::sync::watch;

pub static DUTY_SIGN_BLOCK: &str = "duty_sign_block";
pub static ADVANCE_CONSENSUS_STATE: &str = "advance_consensus_state";
pub static SYNC_EVENT: &str = "sync_event";

/// Singleton manager for `watch::Sender` and `watch::Receiver` used to communicate bail-out
/// contexts.
static BAIL_MANAGER: LazyLock<(
    watch::Sender<Option<String>>,
    watch::Receiver<Option<String>>,
)> = LazyLock::new(|| {
    let (sender, receiver) = watch::channel(None);

    (sender, receiver)
});

/// Publicly accessible `watch::Sender` for broadcasting bail-out context updates.
pub static BAIL_SENDER: LazyLock<watch::Sender<Option<String>>> =
    LazyLock::new(|| BAIL_MANAGER.0.clone());

/// Publicly accessible `watch::Receiver` for subscribing to bail-out context updates.
pub static BAIL_RECEIVER: LazyLock<watch::Receiver<Option<String>>> =
    LazyLock::new(|| BAIL_MANAGER.1.clone());

pub fn check_bail_trigger(ctx: &str) {
    if let Some(val) = BAIL_RECEIVER.borrow().clone() {
        if ctx == val {
            std::process::exit(0);
        }
    }
}
