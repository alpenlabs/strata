use std::sync::LazyLock;

use tokio::sync::watch;

pub static DUTY_SIGN_BLOCK: &str = "duty_sign_block";
pub static ADVANCE_CONSENSUS_STATE: &str = "advance_consensus_state";
pub static SYNC_EVENT: &str = "sync_event";

struct BailWatch {
    sender: watch::Sender<Option<String>>,
    receiver: watch::Receiver<Option<String>>,
}

/// Singleton manager for `watch::Sender` and `watch::Receiver` used to communicate bail-out
/// contexts.
static BAIL_MANAGER: LazyLock<BailWatch> = LazyLock::new(|| {
    let (sender, receiver) = watch::channel(None);

    BailWatch { sender, receiver }
});

/// Publicly accessible `watch::Sender` for broadcasting bail-out context updates.
pub static BAIL_SENDER: LazyLock<watch::Sender<Option<String>>> =
    LazyLock::new(|| BAIL_MANAGER.sender.clone());

/// Publicly accessible `watch::Receiver` for subscribing to bail-out context updates.
pub static BAIL_RECEIVER: LazyLock<watch::Receiver<Option<String>>> =
    LazyLock::new(|| BAIL_MANAGER.receiver.clone());

pub fn check_bail_trigger(ctx: &str) {
    if let Some(val) = BAIL_RECEIVER.borrow().clone() {
        if ctx == val {
            std::process::exit(0);
        }
    }
}
