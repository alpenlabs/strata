use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use alpen_express_bridge_msg::types::BridgeMsgId;
use tokio::sync::Mutex;

pub struct RecentMessageTracker {
    messages: Arc<Mutex<HashMap<BridgeMsgId, u128>>>,
    forget_duration: u64,
}

impl RecentMessageTracker {
    pub fn new(forget_duration: u64) -> Self {
        Self {
            messages: Arc::new(Mutex::new(HashMap::new())),
            forget_duration,
        }
    }

    /// Checks if we should relay the message.
    pub async fn check_should_relay(&self, timestamp: u128, message_id: BridgeMsgId) -> bool {
        let mut msgs = self.messages.lock().await;
        if msgs.contains_key(&message_id) {
            false
        } else {
            msgs.insert(message_id, timestamp);
            true
        }
    }

    /// Clears messages that should be forgotten by now.
    pub async fn clear_stale_messages(&self) {
        let expiration_time = (std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            - Duration::from_secs(self.forget_duration))
        .as_micros();

        let mut msgs = self.messages.lock().await;
        msgs.retain(|_, &mut timestamp| timestamp > expiration_time);
    }
}
