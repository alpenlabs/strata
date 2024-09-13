//! For message routing, deduplication, enforcing operator bandwidth, processing and validation,
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use alpen_express_primitives::relay::types::{BridgeConfig, BridgeMessage};
use alpen_express_status::StatusRx;
use express_storage::ops::bridgemsg::BridgeMsgOps;
use secp256k1::{All, Secp256k1};
use tokio::{select, sync::mpsc, time::interval};
use tracing::*;

use crate::{operator_bandwidth::OperatorBandwidth, recent_msg_tracker::RecentMessageTracker};

/// Manages message validation i.e processing, deduplication, and operator bandwidth enforcement.
///
/// The [`MsgManager`] struct is responsible for handling incoming messages, validating them
/// against the current chain state, enforcing operator bandwidth limits to prevent spamming,
/// and maintaining a record of processed messages to avoid duplication.
pub struct RelayerState {
    /// Tracker to avoid duplicating messages.
    processed_msgs: RecentMessageTracker,

    /// libsecp handle.
    secp: Arc<Secp256k1<All>>,
}

impl RelayerState {
    /// creates new message state
    pub fn new(config: &BridgeConfig) -> Self {
        RelayerState {
            processed_msgs: RecentMessageTracker::new(config.refresh_interval),
            secp: Arc::new(Secp256k1::new()),
        }
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
    ) -> anyhow::Result<()> {
        let message_id = message.compute_id();

        // Check for duplicates
        let timestamp = get_now_micros();
        if !self
            .processed_msgs
            .check_should_relay(timestamp, message_id)
            .await
        {
            trace!(%message_id, "dropping message we've already seen");
            return Ok(());
        }

        let chs_state = status_rx.chs.borrow().clone();
        if let Some(chs_state) = chs_state {
            match alpen_express_primitives::relay::util::verify_bridge_msg_sig(
                &message,
                chs_state.operator_table(),
            ) {
                Ok(()) => {}
                Err(e) => {
                    trace!(err = %e, "dropping invalid message");
                    return Ok(());
                }
            }

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
        self.processed_msgs.clear_stale_messages().await;
        Ok(())
    }
}

pub async fn bridge_msg_worker_task(
    bridge_ops: Arc<BridgeMsgOps>,
    status_rx: Arc<StatusRx>,
    mut msg_state: RelayerState,
    mut message_rx: mpsc::Receiver<BridgeMessage>,
    params: Arc<BridgeConfig>,
) {
    // arbitrary refresh interval to refresh the number of message particular operator can send
    let mut refresh_interval = interval(Duration::from_secs(params.refresh_interval));
    loop {
        select! {
            Some(new_message) = message_rx.recv() => {
                if let Err(e) = msg_state.handle_new_message(new_message, bridge_ops.clone(), status_rx.clone()).await {
                    warn!(err = %e, "failed to handle new message");
                }
            }
            _ = refresh_interval.tick() => {
                // prune old messages that cross the threshold duration
                let duration = get_now_micros_maybe_sub(Some(Duration::from_secs(params.refresh_interval)));
                if let Err(e) = msg_state.prune_old_msg_before(duration, bridge_ops.clone()).await {
                    warn!(err = %e, "Failed to prune old messages");
                }
            }
        }
    }
}

fn get_now_micros() -> u128 {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    duration.as_micros()
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
fn get_now_micros_maybe_sub(sub_duration: Option<Duration>) -> u128 {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
        - sub_duration.unwrap_or(Duration::ZERO);
    duration.as_micros()
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use alpen_express_primitives::relay::types::BridgeMsgId;
    use alpen_test_utils::ArbitraryGenerator;

    use super::*;

    #[tokio::test]
    async fn test_add_and_check_duplicate_message() {
        let processed_msgs = RecentMessageTracker::new(100);

        let message_id: BridgeMsgId = ArbitraryGenerator::new().generate();

        // Initially, the message should not be marked as duplicate
        assert!(!processed_msgs.check_duplicate(&message_id).await);

        // Add the message
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        processed_msgs
            .check_should_relay(timestamp, message_id.clone())
            .await;

        // Now it should be marked as duplicate
        assert!(processed_msgs.check_duplicate(&message_id).await);
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
            .check_should_relay(current_timestamp.as_micros(), message_id.clone())
            .await;
        processed_msgs
            .check_should_relay(old_timestamp.as_micros(), old_message_id.clone())
            .await;

        // Both messages should be considered processed initially
        assert!(processed_msgs.check_duplicate(&message_id).await);
        assert!(processed_msgs.check_duplicate(&old_message_id).await);

        // Clear old messages
        processed_msgs.clear_stale_messages().await;

        // The old message should no longer be considered processed
        assert!(!processed_msgs.check_duplicate(&old_message_id).await);
        // The current message should still be considered processed
        assert!(processed_msgs.check_duplicate(&message_id).await);
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
