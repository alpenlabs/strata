//! For message routing, deduplication, enforcing operator bandwidth, processing and validation,
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use alpen_express_primitives::relay::types::{BridgeMessage, RelayerConfig, Scope};
use alpen_express_status::StatusRx;
use express_storage::ops::bridgemsg::BridgeMsgOps;
use secp256k1::{All, Secp256k1};
use tokio::{select, sync::mpsc, time::interval};
use tracing::*;

use crate::recent_msg_tracker::RecentMessageTracker;

/// Manages message validation i.e processing, deduplication, and operator bandwidth enforcement.
///
/// The [`MsgManager`] struct is responsible for handling incoming messages, validating them
/// against the current chain state, enforcing operator bandwidth limits to prevent spamming,
/// and maintaining a record of processed messages to avoid duplication.
pub struct RelayerState {
    /// Relayer configuration.
    config: RelayerConfig,

    /// Tracker to avoid duplicating messages.
    processed_msgs: RecentMessageTracker,

    /// libsecp handle.
    secp: Arc<Secp256k1<All>>,
}

impl RelayerState {
    /// creates new message state
    pub fn new(config: &RelayerConfig) -> Self {
        Self {
            // TODO make this not need clone
            config: config.clone(),
            processed_msgs: RecentMessageTracker::new(),
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

        // Check if the message is a "misc" message.  If it is, then we should
        // only relay it if we've been set to in the config.
        let is_misc = if let Some(Scope::Misc) = message.try_parse_scope() {
            if !self.config.relay_misc {
                return Ok(());
            }

            debug!("relaying misc message");
            true
        } else {
            false
        };

        let chs_state = status_rx.chs.borrow().clone();
        if let Some(chs_state) = chs_state {
            let sig_res = alpen_express_primitives::relay::util::verify_bridge_msg_sig(
                &message,
                chs_state.operator_table(),
            );

            if let Err(e) = sig_res {
                if !is_misc {
                    trace!(err = %e, "dropping invalid message");
                    return Ok(());
                }
            }

            // Store it in database.
            bridge_ops.write_msg_async(timestamp, message).await?;
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
        before_ts: u128,
        bridge_ops: Arc<BridgeMsgOps>,
    ) -> anyhow::Result<()> {
        // check UNIX time and remove very old messages
        bridge_ops
            .delete_msgs_before_timestamp_async(before_ts)
            .await?;

        // remove from the processed message here
        self.processed_msgs.clear_stale_messages(before_ts).await;
        Ok(())
    }
}

pub async fn bridge_msg_worker_task(
    bridge_ops: Arc<BridgeMsgOps>,
    status_rx: Arc<StatusRx>,
    mut msg_state: RelayerState,
    mut message_rx: mpsc::Receiver<BridgeMessage>,
    config: Arc<RelayerConfig>,
) {
    // arbitrary refresh interval to refresh the number of message particular operator can send
    let mut refresh_interval = interval(Duration::from_secs(config.refresh_interval));
    loop {
        select! {
            Some(new_message) = message_rx.recv() => {
                let bmsg_id = new_message.compute_id();
                trace!(%bmsg_id, "handling new bridge msg");

                if let Err(e) = msg_state.handle_new_message(new_message, bridge_ops.clone(), status_rx.clone()).await {
                    warn!(err = %e, "failed to handle new message");
                }
            }

            _ = refresh_interval.tick() => {
                // prune old messages that cross the threshold duration
                let duration = get_now_micros() - config.stale_duration as u128 * 1_000_000;
                if let Err(e) = msg_state.prune_old_msg_before(duration, bridge_ops.clone()).await {
                    warn!(err = %e, "failed to purge stale messages");
                }
            }
        }
    }
}

fn get_now_micros() -> u128 {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
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
        let processed_msgs = RecentMessageTracker::new();

        let message_id: BridgeMsgId = ArbitraryGenerator::new().generate();

        // Add the message
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros();

        // Initially, the message should not be marked as duplicate
        assert!(processed_msgs.check_should_relay(ts, message_id).await);

        // Now it should be marked as duplicate
        assert!(!processed_msgs.check_should_relay(ts, message_id).await);
    }

    #[tokio::test]
    async fn test_clear_old_messages() {
        let processed_msgs = RecentMessageTracker::new();

        // Create valid BridgeMsgId instances for testing
        let ag = ArbitraryGenerator::new();
        let cur_message_id: BridgeMsgId = ag.generate();
        let old_message_id: BridgeMsgId = ag.generate();
        assert_ne!(cur_message_id, old_message_id);

        // Add messages with different timestamps
        let cur_ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let cur_ts_us = cur_ts.as_micros();
        let old_ts = cur_ts - Duration::from_secs(200);
        let old_ts_us = old_ts.as_micros();

        // Both messages should be considered processed initially
        assert!(
            processed_msgs
                .check_should_relay(cur_ts_us, cur_message_id)
                .await
        );
        assert!(
            processed_msgs
                .check_should_relay(old_ts_us, old_message_id)
                .await
        );

        // Clear old messages
        processed_msgs.clear_stale_messages(cur_ts_us - 1).await;

        // The old message should no longer be considered processed
        assert!(
            processed_msgs
                .check_should_relay(old_ts_us, old_message_id)
                .await
        );

        // The current message should still be considered processed
        assert!(
            !processed_msgs
                .check_should_relay(cur_ts_us, cur_message_id)
                .await
        );
    }

    /*#[tokio::test]
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
    }*/
}
