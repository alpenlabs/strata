//! For message routing, deduplication, enforcing operator bandwidth, processing and validation,
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use alpen_express_bridge_msg::types::{BridgeMessage, BridgeParams};
use alpen_express_status::StatusRx;
use express_storage::ops::bridgemsg::BridgeMsgOps;
use tokio::{select, sync::mpsc, time::interval};
use tracing::{info, warn};

use crate::{operator_bandwidth::OperatorBandwidth, recent_msg_tracker::RecentMessageTracker};

/// Manages message validation i.e processing, deduplication, and operator bandwidth enforcement.
///
/// The [`MsgManager`] struct is responsible for handling incoming messages, validating them
/// against the current chain state, enforcing operator bandwidth limits to prevent spamming,
/// and maintaining a record of processed messages to avoid duplication.
pub struct MsgState {
    /// leaky bucket style request handling method
    operator_bandwidth: OperatorBandwidth,
    /// processed message to avoid duplicate message,
    processed_msgs: RecentMessageTracker,
}

impl MsgState {
    /// creates new message state
    pub fn new(params: Arc<BridgeParams>) -> Self {
        MsgState {
            operator_bandwidth: OperatorBandwidth::new(),
            processed_msgs: RecentMessageTracker::new(params.refresh_interval),
        }
    }

    /// Returns the current Unix timestamp in milliseconds, optionally subtracting a given duration.
    ///
    /// # Arguments
    ///
    /// * `sub_duration` - An optional `Duration` to subtract from the current time.
    ///
    /// # Returns
    ///
    /// * `u64` - The Unix timestamp in milliseconds
    fn get_unix_time(&self, sub_duration: Option<Duration>) -> u128 {
        let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
            - sub_duration.unwrap_or(Duration::ZERO);

        duration.as_micros()
    }

    /// Handles new incoming messages by validating, enforcing bandwidth limits, checking
    /// duplicates, updating the chain state, and storing the message in the database.
    ///
    /// # Arguments
    ///
    /// * `message` - The incoming `BridgeMessage` to be processed.
    async fn handle_new_message(
        &mut self,
        message: BridgeMessage,
        bridge_ops: Arc<BridgeMsgOps>,
        status_rx: Arc<StatusRx>,
        params: Arc<BridgeParams>,
    ) -> anyhow::Result<()> {
        let message_id = message.compute_id()?;

        // Check for duplicates
        if self.processed_msgs.is_duplicate(&message_id).await {
            info!(%message_id, "Message already processed");
            return Ok(());
        }

        // Bandwidth enforcement logic
        let source_id = message.source_id();
        if self
            .update_bandwidth_counter(source_id, params.bandwidth)
            .await
        {
            warn!(%source_id, "Bandwidth limit crossed (possible spamming)");
            return Ok(());
        }
        let chs_state = status_rx.chs.borrow().clone();
        if let Some(chs_state) = chs_state {
            match chs_state.operator_table().get_operator(source_id) {
                Some(operator) => {
                    if !alpen_express_bridge_msg::utils::check_signature_validity(
                        *operator.signing_pk().as_ref(),
                        message.payload(),
                        *message.signature().as_ref(),
                    )? {
                        println!("Signature is invalid");
                        return Ok(());
                    }
                }
                None => {
                    return Ok(());
                }
            }

            // insert it into processed_message
            //
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros();

            self.processed_msgs
                .add_message(timestamp, message_id.clone())
                .await;

            // update the operator table, to enforce the bandwidth
            self.operator_bandwidth.increment(source_id).await;
            //  store them in database
            bridge_ops.write_msg_blocking(timestamp, message)?;
        }

        Ok(())
    }

    /// Prunes old messages that are older than the specified threshold from the database and
    /// internal state.
    ///
    /// # Arguments
    ///
    /// * `time_before` - The cutoff Unix timestamp; messages older than this will be pruned.
    async fn prune_old_msg_before(
        &mut self,
        time_before: u128,
        bridge_ops: Arc<BridgeMsgOps>,
    ) -> anyhow::Result<()> {
        // check UNIX time and remove very old messages
        bridge_ops.delete_msgs_before_timestamp_blocking(time_before)?;

        // remove from the processed message here
        self.processed_msgs.clear_old_messages().await;
        Ok(())
    }

    /// Enforces operator bandwidth limits by checking if the source ID has exceeded the allowed
    /// threshold.
    ///
    /// # Arguments
    ///
    /// * `source_id` - The identifier of the operator to check.
    async fn update_bandwidth_counter(&self, source_id: u32, bandwidth: u32) -> bool {
        // Check if the source_id is greater than BANDWIDTH
        if self.operator_bandwidth.get(source_id).await > bandwidth {
            return true;
        }
        false
    }
}

pub async fn bridge_msg_worker_task(
    bridge_ops: Arc<BridgeMsgOps>,
    status_rx: Arc<StatusRx>,
    mut msg_state: MsgState,
    mut message_rx: mpsc::Receiver<BridgeMessage>,
    params: Arc<BridgeParams>,
) {
    // arbitrary refresh interval to refresh the number of message particular operator can send
    let mut refresh_interval = interval(Duration::from_secs(params.refresh_interval));
    loop {
        select! {
            Some(new_message) = message_rx.recv() => {
                if let Err(e) = msg_state.handle_new_message(new_message, bridge_ops.clone(), status_rx.clone(), params.clone()).await {
                    warn!(err = %e, "Failed to handle new message");
                }
            }
            _ = refresh_interval.tick() => {
                // clear the operator bandwidth
                msg_state.operator_bandwidth.clear().await;

                // prune old messages that cross the threshold duration
                let duration = msg_state.get_unix_time(Some(Duration::from_secs(params.refresh_interval)));
                if let Err(e) = msg_state.prune_old_msg_before(duration, bridge_ops.clone()).await {
                    warn!(err = %e, "Failed to prune old messages");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use alpen_express_bridge_msg::types::BridgeMsgId;
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;

    #[tokio::test]
    async fn test_add_and_check_duplicate_message() {
        let processed_msgs = RecentMessageTracker::new(100);

        let message_id: BridgeMsgId = ArbitraryGenerator::new().generate();

        // Initially, the message should not be marked as duplicate
        assert!(!processed_msgs.is_duplicate(&message_id).await);

        // Add the message
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        processed_msgs
            .add_message(timestamp, message_id.clone())
            .await;

        // Now it should be marked as duplicate
        assert!(processed_msgs.is_duplicate(&message_id).await);
    }

    #[tokio::test]
    async fn test_clear_old_messages() {
        let processed_msgs = RecentMessageTracker::new(100);

        // Create valid BridgeMsgId instances for testing
        let message_id: BridgeMsgId = ArbitraryGenerator::new().generate();
        let old_message_id: BridgeMsgId = ArbitraryGenerator::new().generate();

        // Add messages with different timestamps
        let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let old_timestamp = current_timestamp - Duration::from_secs(200);

        processed_msgs
            .add_message(current_timestamp.as_micros(), message_id.clone())
            .await;
        processed_msgs
            .add_message(old_timestamp.as_micros(), old_message_id.clone())
            .await;

        // Both messages should be considered processed initially
        assert!(processed_msgs.is_duplicate(&message_id).await);
        assert!(processed_msgs.is_duplicate(&old_message_id).await);

        // Clear old messages
        processed_msgs.clear_old_messages().await;

        // The old message should no longer be considered processed
        assert!(!processed_msgs.is_duplicate(&old_message_id).await);
        // The current message should still be considered processed
        assert!(processed_msgs.is_duplicate(&message_id).await);
    }

    #[tokio::test]
    async fn test_increment_and_reset_bandwidth() {
        let operator_bandwidth = OperatorBandwidth::new();
        // Create a valid OperatorIdx for testing
        let operator_id: u32 = 1;

        // Initially, the bandwidth for the operator should be 0
        assert_eq!(operator_bandwidth.get(operator_id).await, 0);

        // Increment the bandwidth
        operator_bandwidth.increment(operator_id).await;
        assert_eq!(operator_bandwidth.get(operator_id).await, 1);

        // Increment again
        operator_bandwidth.increment(operator_id).await;
        assert_eq!(operator_bandwidth.get(operator_id).await, 2);

        // Reset the bandwidth
        operator_bandwidth.reset(operator_id).await;
        assert_eq!(operator_bandwidth.get(operator_id).await, 0);
    }

    #[tokio::test]
    async fn test_clear_bandwidth() {
        let operator_bandwidth = OperatorBandwidth::new();
        // Create valid OperatorIdx instances for testing
        let operator_id_1 = 10;
        let operator_id_2 = 20; // Use a different ID for clarity

        // Increment bandwidth for two different operators
        operator_bandwidth.increment(operator_id_1).await;
        operator_bandwidth.increment(operator_id_2).await;

        assert_eq!(operator_bandwidth.get(operator_id_1).await, 1);
        assert_eq!(operator_bandwidth.get(operator_id_2).await, 1);

        // Clear all records
        operator_bandwidth.clear().await;

        // After clearing, the bandwidth for both operators should be 0
        assert_eq!(operator_bandwidth.get(operator_id_1).await, 0);
        assert_eq!(operator_bandwidth.get(operator_id_2).await, 0);
    }
}
