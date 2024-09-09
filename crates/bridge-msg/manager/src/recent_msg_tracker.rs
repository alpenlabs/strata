use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use alpen_express_bridge_msg::types::BridgeMsgId;
use tokio::sync::RwLock;

#[derive(Default)]
pub(crate) struct RecentMessageTracker {
    messages: Arc<RwLock<HashMap<BridgeMsgId, u128>>>,
    refresh_interval: u64,
}

impl RecentMessageTracker {
    pub(crate) fn new(refresh_interval: u64) -> Self {
        Self {
            messages: Arc::new(RwLock::new(HashMap::new())),
            refresh_interval,
        }
    }

    pub(crate) async fn add_message(&self, timestamp: u128, message_id: BridgeMsgId) {
        self.messages.write().await.insert(message_id, timestamp);
    }

    pub(crate) async fn is_duplicate(&self, message_id: &BridgeMsgId) -> bool {
        let messages = self.messages.read().await;
        messages.contains_key(message_id)
    }

    pub(crate) async fn clear_old_messages(&self) {
        let expiration_time = (std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            - Duration::from_secs(self.refresh_interval))
        .as_micros();

        self.messages
            .write()
            .await
            .retain(|_, &mut timestamp| timestamp > expiration_time);
    }
}
