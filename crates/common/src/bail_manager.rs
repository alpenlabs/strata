use std::sync::LazyLock;
use tokio::sync::watch;

pub enum BailContext {
    AfterSyncEvent,
}

static BAIL_MANAGER: LazyLock<(watch::Sender<String>, watch::Receiver<String>)> = LazyLock::new(|| {
    let (sender, receiver) = watch::channel(String::new());

    (sender, receiver)
});

pub static BAIL_SENDER: LazyLock<watch::Sender<String>> = LazyLock::new(|| {
    BAIL_MANAGER.0.clone()
});
pub static BAIL_RECEIVER: LazyLock<watch::Receiver<String>> = LazyLock::new(|| {
    BAIL_MANAGER.1.clone()
});


