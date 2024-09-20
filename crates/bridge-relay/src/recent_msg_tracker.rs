use std::{collections::HashMap, sync::Arc};

use strata_primitives::relay::types::BridgeMsgId;
use tokio::sync::Mutex;

pub struct RecentMessageTracker {
    messages: Arc<Mutex<HashMap<BridgeMsgId, u128>>>,
}

impl RecentMessageTracker {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Checks if we should relay the message.
    pub async fn check_should_relay(&self, cur_timestamp: u128, message_id: BridgeMsgId) -> bool {
        let mut msgs = self.messages.lock().await;
        if msgs.contains_key(&message_id) {
            return false;
        }

        msgs.insert(message_id, cur_timestamp);
        true
    }

    /// Clears messages that should be forgotten by now.
    pub async fn clear_stale_messages(&self, before_ts: u128) {
        let mut msgs = self.messages.lock().await;
        msgs.retain(|_, &mut timestamp| timestamp > before_ts);
    }
}
