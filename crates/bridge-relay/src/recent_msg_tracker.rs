use std::{collections::HashMap, sync::Arc};

use alpen_express_primitives::relay::types::BridgeMsgId;
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

    pub fn forget_duration_us(&self) -> u128 {
        self.forget_duration as u128
    }

    /// Checks if we should relay the message.
    pub async fn check_should_relay(&self, cur_timestamp: u128, message_id: BridgeMsgId) -> bool {
        let mut msgs = self.messages.lock().await;
        if let Some(last_ts) = msgs.get(&message_id) {
            let age = cur_timestamp - last_ts;
            if (age as u64) < self.forget_duration {
                return false;
            }
        }

        msgs.insert(message_id, cur_timestamp);
        true
    }

    /// Clears messages that should be forgotten by now.
    pub async fn clear_stale_messages(&self, cur_timestamp: u128) {
        let expiration_time = cur_timestamp - self.forget_duration as u128;
        let mut msgs = self.messages.lock().await;
        msgs.retain(|_, &mut timestamp| timestamp > expiration_time);
    }
}
